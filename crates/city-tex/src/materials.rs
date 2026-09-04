//! The material painters. One function per [`Material`], all of them tileable at
//! [`crate::TILE`] texels and deterministic in `seed`.
//!
//! Conventions:
//! * Noise is sampled on the **torus**: coordinate `lat * freq` with `lat` in
//!   `0..1`, so every lattice frequency is a multiple of the tile period and the
//!   tiles repeat without a seam (see [`crate::noise`]).
//! * Painters use [`Texture::blend`], so overlapping details (cracks over joints,
//!   stains over bricks) mix instead of clobbering each other.
//! * All randomness comes from `city_math::hash2d` with the material `seed` — the
//!   same seed always paints byte-identical pixels.

use crate::{
    luts::GradientLut,
    noise::fbm,
    palette as pal,
    Material, Texture, TILE,
};

/// Paint `m` into a fresh `TILE × TILE` texture.
pub fn generate(m: Material, seed: u64) -> Texture {
    let mut t = Texture::new(TILE, TILE);
    match m {
        Material::Asphalt => asphalt(&mut t, seed),
        Material::Concrete => concrete(&mut t, seed),
        Material::Sidewalk => sidewalk(&mut t, seed),
        Material::Grass => grass(&mut t, seed),
        Material::Brick => brick(&mut t, seed),
        Material::Plaster => plaster(&mut t, seed),
        Material::RoofGravel => roof_gravel(&mut t, seed),
        Material::Metal => metal(&mut t, seed),
        Material::RoadPaintWhite => road_paint(&mut t, seed, pal::PAINT_WHITE),
        Material::RoadLineYellow => road_paint(&mut t, seed, pal::PAINT_YELLOW),
    }
    t
}

/// Torus-sampled fbm at tile coordinates `x, y` (in texels).
///
/// The domain spans exactly one period of every octave lattice (`freq`, `2*freq`,
/// ... cells per tile), so the field repeats seamlessly when the tile repeats.
#[inline]
fn fbm_tile(x: u32, y: u32, freq: i32, octaves: u32, seed: u64) -> f32 {
    let lat = x as f32 / TILE as f32;
    let lon = y as f32 / TILE as f32;
    fbm(lat, lon, freq, octaves, seed)
}

/// Hash speckle in `0..=1`, one draw per texel.
#[inline]
fn speckle(x: u32, y: u32, seed: u64) -> f32 {
    city_math::hash::hash2d_unit(x as i32, y as i32, seed)
}

// ---------------------------------------------------------------------------
// asphalt family
// ---------------------------------------------------------------------------

/// Asphalt: dark bitumen base, dense aggregate speckle, sparse bright chippings
/// and slow oil stains.
pub fn asphalt(t: &mut Texture, seed: u64) {
    for y in 0..t.height {
        for x in 0..t.width {
            let n = fbm_tile(x, y, 8, 3, seed);
            let mut c = crate::scale3(pal::ASPHALT, 0.80 + 0.40 * n);
            let sp = speckle(x, y, seed);
            if sp > 0.86 {
                c = crate::mix3(c, pal::ASPHALT_GRAIN, (sp - 0.86) * 3.0);
            } else if sp < 0.08 {
                c = crate::mix3(c, pal::ASPHALT_DARK, 0.5);
            }
            let oil = fbm_tile(x, y, 4, 2, seed ^ 0x011);
            if oil > 0.66 {
                c = crate::mix3(c, pal::OIL_STAIN, city_math::smoothstep(0.66, 0.92, oil) * 0.55);
            }
            t.set(x, y, c);
        }
    }
}

/// Worn road paint (white line or centre yellow): paint colour, dirt and chipping
/// that lets the carriageway through.
pub fn road_paint(t: &mut Texture, seed: u64, colour: [u8; 3]) {
    for y in 0..t.height {
        for x in 0..t.width {
            let dirt = fbm_tile(x, y, 8, 3, seed ^ 0xdea);
            let mut c = crate::mix3(colour, pal::ASPHALT, city_math::smoothstep(0.45, 0.95, dirt) * 0.6);
            let chip = speckle(x, y, seed ^ 0xc4);
            if chip > 0.78 {
                c = crate::mix3(c, pal::ASPHALT, (chip - 0.78) * 3.0);
            }
            t.set(x, y, c);
        }
    }
}

// ---------------------------------------------------------------------------
// concrete family
// ---------------------------------------------------------------------------

/// Plain concrete: mottled grey with sparse dark specks.
pub fn concrete(t: &mut Texture, seed: u64) {
    for y in 0..t.height {
        for x in 0..t.width {
            let n = fbm_tile(x, y, 16, 3, seed);
            let mut c = crate::scale3(pal::CONCRETE, 0.88 + 0.26 * n);
            if speckle(x, y, seed ^ 7) > 0.955 {
                c = crate::mix3(c, pal::CONCRETE_DARK, 0.6);
            }
            t.set(x, y, c);
        }
    }
}

/// Sidewalk: square slabs with grooved joints, per-slab tone drift, gum stains
/// and a meandering crack network on top.
pub fn sidewalk(t: &mut Texture, seed: u64) {
    const SLABS: u32 = 4; // joint every TILE/SLABS texels
    let slab = TILE / SLABS;
    for y in 0..t.height {
        for x in 0..t.width {
            let gx = x / slab;
            let gy = y / slab;
            let tone = speckle(gx, gy, seed);
            let n = fbm_tile(x, y, 16, 2, seed ^ 0x5bd);
            let mut c = crate::scale3(pal::SIDEWALK, 0.86 + 0.16 * tone + 0.14 * n);
            // 2-texel groove on the slab borders
            let ix = x % slab;
            let iy = y % slab;
            if ix < 2 || iy < 2 {
                c = crate::mix3(c, pal::SIDEWALK_JOINT, 0.8);
            }
            t.set(x, y, c);
        }
    }
    // cracks across the whole sidewalk (seamless, see crack_network)
    // crack network across the sidewalk (wraps, so it is seamless)
    let network = crack_network(seed, 10);
    for &(x, y, a) in &network {
        t.blend(x, y, pal::SIDEWALK_CRACK, a);
    }
}

// ---------------------------------------------------------------------------
// grass
// ---------------------------------------------------------------------------

/// Grass: clump noise × blade noise, with dry straw patches on a slow ramp.
pub fn grass(t: &mut Texture, seed: u64) {
    let dry_ramp = GradientLut::from_stops(&[pal::GRASS, pal::GRASS, pal::GRASS_DRY]).unwrap();
    for y in 0..t.height {
        for x in 0..t.width {
            let clump = fbm_tile(x, y, 8, 2, seed);
            let blade = fbm_tile(x, y, 32, 2, seed ^ 0xb1);
            let mut c = crate::mix3(
                pal::GRASS_DARK,
                pal::GRASS_LIGHT,
                0.15 + 0.5 * blade + 0.35 * clump,
            );
            let dry = fbm_tile(x, y, 4, 2, seed ^ 0xd4);
            if dry > 0.62 {
                let a = city_math::smoothstep(0.62, 0.88, dry) * 0.75;
                c = crate::mix3(c, dry_ramp.sample(0.5 + 0.5 * blade), a);
            }
            t.set(x, y, c);
        }
    }
}

// ---------------------------------------------------------------------------
// facades
// ---------------------------------------------------------------------------

/// Brick in stretcher bond: per-brick tone, burnt outliers, mortar joints.
pub fn brick(t: &mut Texture, seed: u64) {
    const COURSES: u32 = 16; // brick rows per tile
    const COLUMNS: u32 = 8; // brick columns per tile
    let rh = TILE / COURSES;
    let rw = TILE / COLUMNS;
    for y in 0..t.height {
        for x in 0..t.width {
            let row = y / rh;
            // alternate courses shift half a brick (stretcher bond)
            let off = if row % 2 == 1 { rw / 2 } else { 0 };
            let bx = (x + off) % TILE / rw;
            let mortar = (x + off) % rw < 2 || y % rh < 2;
            let tone = speckle(bx, row, seed);
            let base = crate::mix3(pal::BRICK_DARK, pal::BRICK_LIGHT, 0.25 + 0.7 * tone);
            let burnt = speckle(bx, row, seed ^ 0xb2) > 0.94;
            let c = if mortar {
                let n = fbm_tile(x, y, 32, 2, seed ^ 0xf0);
                crate::scale3(pal::MORTAR, 0.9 + 0.2 * n)
            } else if burnt {
                pal::BRICK_BURNT
            } else {
                base
            };
            t.set(x, y, c);
        }
    }
}

/// Plaster / stucco: soft mottling, sand grain and vertical dirt streaks.
pub fn plaster(t: &mut Texture, seed: u64) {
    for y in 0..t.height {
        for x in 0..t.width {
            // narrow mottle band: soft mottle, never a hard stamp
            let m = fbm_tile(x, y, 8, 3, seed);
            let mut c = crate::scale3(pal::PLASTER, 0.94 + 0.14 * m);
            // rain streaks: the lattice is squeezed hard along U (period TILE/2 in
            // x, free in y), so they read as vertical runs, not speckle
            let streak = fbm(x as f32 * 0.25, y as f32 * 0.03, 1, 2, seed ^ 0x57);
            let a = city_math::smoothstep(0.60, 0.80, streak) * 0.25;
            c = crate::mix3(c, pal::PLASTER_DARK, a);
            t.set(x, y, c);
        }
    }
}

/// Roof gravel: pale chippings on bitumen, a few bright flecks and puddle sheen.
pub fn roof_gravel(t: &mut Texture, seed: u64) {
    for y in 0..t.height {
        for x in 0..t.width {
            let n = fbm_tile(x, y, 32, 2, seed);
            let mut c = crate::mix3(pal::GRAVEL_DARK, pal::GRAVEL_LIGHT, 0.25 + 0.7 * n);
            let sp = speckle(x, y, seed);
            if sp > 0.955 {
                c = crate::mix3(c, pal::GRAVEL_BRIGHT, 0.85);
            }
            let p = fbm_tile(x, y, 4, 2, seed ^ 0x97);
            if p > 0.78 {
                c = crate::mix3(c, pal::GRAVEL_WET, city_math::smoothstep(0.78, 0.95, p));
            }
            t.set(x, y, c);
        }
    }
}

// ---------------------------------------------------------------------------
// metal
// ---------------------------------------------------------------------------

/// Brushed metal: streaks along U (constant per row, wrapping every
/// `BRUSH_LINE` texels), faint rust freckles.
pub const BRUSH_LINE: u32 = 32;
pub fn metal(t: &mut Texture, seed: u64) {
    for y in 0..t.height {
        // one brushed streak line per row: the streak value is constant along the
        // row and drawn from a lattice that wraps over `BRUSH_PERIOD` texels
        let line = city_math::hash::hash2d_unit(0, y as i32, seed);
        for x in 0..t.width {
            let cell = ((x / 4) % BRUSH_LINE) as i32;
            let g = city_math::hash::hash2d_unit(cell, y as i32, seed);
            let mut c = crate::scale3(pal::METAL, 0.92 + 0.30 * (line + 0.18 * g));
            let rust = fbm_tile(x, y, 8, 2, seed ^ 0x33);
            if rust > 0.74 {
                c = crate::mix3(c, pal::RUST, city_math::smoothstep(0.74, 0.92, rust) * 0.65);
            }
            t.set(x, y, c);
        }
    }
}

// ---------------------------------------------------------------------------
// crack network (shared by sidewalk and roof)
// ---------------------------------------------------------------------------

/// A stamped crack: `(x, y, alpha)` per texel, wrapping over the tile.
pub(crate) fn crack_network(seed: u64, count: u32) -> Vec<(u32, u32, f32)> {
    const DIRS: [(i32, i32); 8] = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    let size = TILE as i32;
    let mut out = Vec::new();
    for k in 0..count {
        let h = city_math::hash12(k as u64, seed);
        let mut x = (h % size as u64) as i32;
        let mut y = ((h >> 32) % size as u64) as i32;
        let mut dir = (h >> 16) as usize % 8;
        let len = 48 + (h >> 24) % 64;
        for step in 0..len {
            // jagged but mostly straight: rare ±45° turns
            let turn = city_math::mix(h ^ step ^ (k as u64)) % 6;
            if turn == 0 {
                dir = (dir + 1) % 8;
            } else if turn == 1 {
                dir = (dir + 7) % 8;
            }
            let (dx, dy) = DIRS[dir];
            x += dx;
            y += dy;
            // cracks fade along their length; coordinates wrap over the torus
            let a = 0.85 * (1.0 - (step as f32 / len as f32) * 0.5);
            out.push((
                x.rem_euclid(size) as u32,
                y.rem_euclid(size) as u32,
                a,
            ));
        }
    }
    out
}
