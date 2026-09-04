//! Tileable value noise, built on the shared kernel's [`city_math::hash`].
//!
//! Two families live here:
//!
//! * [`value_noise`] / [`fbm`] sample the **infinite hash lattice** — any
//!   coordinates, any frequency. Each octave has its own lattice (seeded per
//!   octave), so the summed field has period `1/oct_freq`, not `1.0`.
//! * [`NoiseLut`] pre-computes one tile's lattice and **wraps lattice cells with
//!   the tile period**. That wrap is what makes a texture tile seamlessly: octave
//!   `k` hashes a `freq`-cell lattice whose cells wrap at `freq`, so octave `k`
//!   (and the sum) closes exactly at the tile border. This is the construction
//!   `materials::fbm_tile` uses for every material.

use city_math::hash::hash2d_unit;

/// Integer-lattice value noise in `0..=1` (no pre-computation, any coordinates).
///
/// At a lattice corner the result is exactly the cell hash; between corners the
/// four corner values are bilinearly blended.
#[inline]
pub fn value_noise(x: f32, y: f32, seed: u64) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let fx = x - xi;
    let fy = y - yi;
    let (x0, y0) = (xi as i32, yi as i32);
    let a = hash2d_unit(x0, y0, seed);
    let b = hash2d_unit(x0 + 1, y0, seed);
    let c = hash2d_unit(x0, y0 + 1, seed);
    let d = hash2d_unit(x0 + 1, y0 + 1, seed);
    let ab = a + (b - a) * fx;
    let cd = c + (d - c) * fx;
    ab + (cd - ab) * fy
}

/// `n` octaves of [`value_noise`], renormalised to `0..=1`.
///
/// Octave `k` samples frequency `oct_freq * 2^k` with octave seed `seed ^ k`.
/// The field is periodic with period `1 / oct_freq` on the x/y axes.
pub fn fbm(x: f32, y: f32, oct_freq: i32, octaves: u32, seed: u64) -> f32 {
    let mut sum = 0.0f32;
    let mut norm = 0.0f32;
    let mut amp = 1.0f32;
    let mut f = oct_freq.max(1) as f32;
    for o in 0..octaves.max(1) {
        sum += value_noise(x * f, y * f, seed ^ (o as u64)) * amp;
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

/// A pre-computed lattice for one square tile of `size` texels at frequency
/// `freq` (which must divide `size`). Cells wrap with the lattice, so sampling
/// the tile at `0` and at `size` gives the same value: a seamless torus.
#[derive(Clone, Debug, PartialEq)]
pub struct NoiseLut {
    size: u32,
    freq: u32,
    values: Vec<f32>,
}

impl NoiseLut {
    /// Build the lattice.
    ///
    /// # Panics
    /// If `freq < 1`, `freq > size` or `size % freq != 0`.
    pub fn new(size: u32, freq: u32, seed: u64) -> NoiseLut {
        assert!(freq >= 1, "NoiseLut: freq must be >= 1");
        assert!(
            freq <= size && size.is_multiple_of(freq),
            "NoiseLut: freq {freq} must divide size {size}"
        );
        let f = freq as usize;
        let values = (0..(f * f))
            .map(|i| {
                let x = (i % f) as i32;
                let y = (i / f) as i32;
                hash2d_unit(x, y, seed)
            })
            .collect();
        NoiseLut {
            size,
            freq,
            values,
        }
    }

    pub fn size(&self) -> u32 {
        self.size
    }
    pub fn freq(&self) -> u32 {
        self.freq
    }

    /// Raw lattice noise at tile coordinates `u, v` in `0..size`, bilinearly
    /// smoothed, wrapping at the edges (`REPEAT` semantics).
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let scale = self.freq as f32 / self.size as f32;
        let x = u * scale;
        let y = v * scale;
        let fx = x - x.floor();
        let fy = y - y.floor();
        let f = self.freq as i32;
        let x0 = (x.floor() as i32).rem_euclid(f) as usize;
        let y0 = (y.floor() as i32).rem_euclid(f) as usize;
        let x1 = (x0 + 1) % self.freq as usize;
        let y1 = (y0 + 1) % self.freq as usize;
        let at = |cx: usize, cy: usize| self.values[cy * self.freq as usize + cx];
        let a = at(x0, y0);
        let b = at(x1, y0);
        let c = at(x0, y1);
        let d = at(x1, y1);
        let ab = a + (b - a) * fx;
        let cd = c + (d - c) * fx;
        ab + (cd - ab) * fy
    }

    /// `n` octaves of the wrapped lattice, renormalised to `0..=1`.
    ///
    /// Octave `k` uses frequency `freq * 2^k` (still dividing `size`), hashed with
    /// a per-octave seed and wrapped at the octave lattice, so the sum is seamless
    /// across the tile.
    pub fn fbm(&self, u: f32, v: f32, octaves: u32) -> f32 {
        let mut sum = 0.0f32;
        let mut norm = 0.0f32;
        let mut amp = 1.0f32;
        let mut f = self.freq;
        for k in 0..octaves.max(1) {
            if f > self.size || !self.size.is_multiple_of(f) {
                break;
            }
            let s = f as f32 / self.size as f32;
            sum += sample_wrapped(u * s, v * s, f, self.seed_of_oct(k)) * amp;
            norm += amp;
            amp *= 0.5;
            f *= 2;
        }
        sum / norm
    }

    fn seed_of_oct(&self, octave: u32) -> u64 {
        city_math::hash12(self.freq as u64, octave as u64)
    }
}

/// Sample a lattice of `freq` cells over the unit square, wrapping cells with the
/// lattice period so the octave tiles seamlessly.
fn sample_wrapped(x: f32, y: f32, freq: u32, seed: u64) -> f32 {
    let fx = x - x.floor();
    let fy = y - y.floor();
    let f = freq as i64;
    let x0 = (x.floor() as i64).rem_euclid(f) as i32;
    let y0 = (y.floor() as i64).rem_euclid(f) as i32;
    let x1 = ((x0 as u32 + 1) % freq) as i32;
    let y1 = ((y0 as i64 + 1).rem_euclid(f)) as i32;
    let a = hash2d_unit(x0, y0, seed);
    let b = hash2d_unit(x1, y0, seed);
    let c = hash2d_unit(x0, y1, seed);
    let d = hash2d_unit(x1, y1, seed);
    let ab = a + (b - a) * fx;
    let cd = c + (d - c) * fx;
    ab + (cd - ab) * fy
}
