//! # city-tex
//!
//! Procedural material textures — **no image files anywhere**. Every surface of the
//! city (asphalt, concrete kerbs, cracked sidewalk concrete, grass, brick and plaster
//! facades, roof gravel, brushed metal, road paint) is painted here, in pure Rust,
//! into small RGBA8 tiles that the renderer repeats across the geometry.
//!
//! Design notes:
//! * Every generator is a pure function of `(texel, seed)`: the same seed always
//!   produces byte-identical tiles, so native tests and the browser screenshot tests
//!   stay comparable.
//! * Tiles are generated on a **torus** (all noise and crack segments wrap), so a
//!   repeated tile shows no seams.
//! * Output bytes are sRGB-encoded (`0..=255`), ready for `texImage2D` as RGBA8.
//! * The crate is `forbid(unsafe_code)` and depends on nothing but `city-math`.

#![forbid(unsafe_code)]

pub mod luts;
pub mod materials;
pub mod noise;
pub mod palette;

pub use luts::{GradientLut, LutError};
pub use materials::generate;
pub use noise::NoiseLut;

/// Default edge length of every generated tile, in texels.
pub const TILE: u32 = 128;

/// Every material the renderer knows about; each has its own painter in
/// [`materials`]. The order matches the order the renderer binds its texture units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    Asphalt,
    Concrete,
    Sidewalk,
    Grass,
    Brick,
    Plaster,
    RoofGravel,
    Metal,
    RoadPaintWhite,
    RoadLineYellow,
}

/// All materials, in sampler-slot order.
pub const ALL_MATERIALS: [Material; 10] = [
    Material::Asphalt,
    Material::Concrete,
    Material::Sidewalk,
    Material::Grass,
    Material::Brick,
    Material::Plaster,
    Material::RoofGravel,
    Material::Metal,
    Material::RoadPaintWhite,
    Material::RoadLineYellow,
];

/// A CPU-generated RGBA8 texture (row-major, `w * h * 4` bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Texture {
    /// An opaque, fully black texture of the given size.
    pub fn new(w: u32, h: u32) -> Texture {
        Texture {
            width: w,
            height: h,
            pixels: vec![255u8; (w as usize) * (h as usize) * 4],
        }
    }

    /// Number of texels.
    #[inline]
    pub fn len(&self) -> usize {
        (self.width as usize) * (self.height as usize)
    }

    /// `true` when the texture holds no texels.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Write one texel, coordinates wrapping like a `REPEAT` sampler.
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, c: [u8; 3]) {
        let x = x % self.width;
        let y = y % self.height;
        let i = ((y as usize) * (self.width as usize) + x as usize) * 4;
        self.pixels[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
    }

    /// Alpha-composite `c` over the texel at `(x, y)` (wrapping). Used by painters
    /// that overlay details (cracks, stains, chippings) on an already-painted base.
    #[inline]
    pub fn blend(&mut self, x: u32, y: u32, c: [u8; 3], a: f32) {
        let a = city_math::saturate(a);
        if a <= 0.0 {
            return;
        }
        let x = x % self.width;
        let y = y % self.height;
        let i = ((y as usize) * (self.width as usize) + x as usize) * 4;
        let p = &mut self.pixels[i..i + 4];
        for k in 0..3 {
            p[k] = (p[k] as f32 + (c[k] as f32 - p[k] as f32) * a) as u8;
        }
        p[3] = 255;
    }

    /// Read one texel, coordinates wrapping like a `REPEAT` sampler.
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> [u8; 3] {
        let x = x % self.width;
        let y = y % self.height;
        let i = ((y as usize) * (self.width as usize) + x as usize) * 4;
        [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2]]
    }

    /// bilinear-filtered fetch in *tile* space (`u`, `v` in texels, wrapping).
    pub fn sample_bilinear(&self, u: f32, v: f32) -> [f32; 3] {
        let fx = u - u.floor();
        let t = city_math::saturate(v - v.floor());
        let x0 = (u.floor() as i64).rem_euclid(self.width as i64) as u32;
        let y0 = (v.floor() as i64).rem_euclid(self.height as i64) as u32;
        let x1 = (x0 + 1) % self.width;
        let y1 = (y0 + 1) % self.height;
        let a = self.get(x0, y0);
        let b = self.get(x1, y0);
        let c = self.get(x0, y1);
        let d = self.get(x1, y1);
        let mix = |k: usize| {
            let ab = a[k] as f32 + (b[k] as f32 - a[k] as f32) * fx;
            let cc = c[k] as f32 + (d[k] as f32 - c[k] as f32) * fx;
            ab + (cc - ab) * t
        };
        [mix(0), mix(1), mix(2)]
    }

    /// Mean colour of every texel, in `0..=1`.
    pub fn average(&self) -> [f32; 3] {
        let n = self.len();
        if n == 0 {
            return [0.0, 0.0, 0.0];
        }
        let (mut r, mut g, mut b) = (0u64, 0u64, 0u64);
        for px in self.pixels.chunks_exact(4) {
            r += px[0] as u64;
            g += px[1] as u64;
            b += px[2] as u64;
        }
        [
            r as f32 / n as f32 / 255.0,
            g as f32 / n as f32 / 255.0,
            b as f32 / n as f32 / 255.0,
        ]
    }

    /// Mean absolute channel difference between corresponding texels of two textures.
    /// `0.0` means the tiles are identical.
    pub fn mean_diff(&self, other: &Texture) -> f32 {
        if self.width != other.width || self.height != other.height {
            return 1.0;
        }
        let n = self.len();
        if n == 0 {
            return 0.0;
        }
        let mut sum = 0u64;
        for (a, b) in self
            .pixels
            .chunks_exact(4)
            .zip(other.pixels.chunks_exact(4))
        {
            for k in 0..3 {
                sum += (a[k] as i32 - b[k] as i32).unsigned_abs() as u64;
            }
        }
        sum as f32 / (n as f32 * 3.0 * 255.0)
    }
}

/// Linear (not sRGB) fade between two colours; kept here so materials stay readable.
#[inline]
pub(crate) fn mix3(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = city_math::saturate(t);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}

/// Multiply a colour by a scalar and encode back to bytes.
#[inline]
pub(crate) fn scale3(c: [u8; 3], k: f32) -> [u8; 3] {
    [
        (c[0] as f32 * k).round().clamp(0.0, 255.0) as u8,
        (c[1] as f32 * k).round().clamp(0.0, 255.0) as u8,
        (c[2] as f32 * k).round().clamp(0.0, 255.0) as u8,
    ]
}
