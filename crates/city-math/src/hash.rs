//! Deterministic integer hashing used for hashing spatial cells and ids.
//!
//! These are the *only* sources of "randomness" besides [`crate::Rng`], so a city
//! generated from the same inputs is byte-identical everywhere.

/// Mix a 64 bit value (splitmix64 finaliser).
#[inline]
pub fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_6d53_d7ce_6ee0);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Combine two `u64` into one (Szembek style, good for `(a, b)` keys).
#[inline]
pub fn hash12(a: u64, b: u64) -> u64 {
    let k = 0x9E37_79B9_7F4A_7C15u64;
    mix(a ^ k) ^ mix(b.rotate_left(32).wrapping_add(k))
}

/// Hash a 2D integer lattice point together with a salt.
///
/// Used by the procedural texture/material code and by the city generator to pick
/// stable per-lot variations. Both coordinates are mixed through [`mix`] so that
/// neighbouring cells decorrelate — without that, value noise over this lattice
/// would not be a noise field at all.
#[inline]
pub fn hash2d(x: i32, y: i32, salt: u64) -> u64 {
    let mut h = mix(salt ^ 0x517c_1ce1_90cb_2787);
    h = mix(h ^ ((x as i64) as u64).wrapping_mul(0x0100_0000_01b3));
    h = mix(h ^ ((y as i64) as u64).wrapping_mul(0x2722_0a95_3b13_f14f));
    mix(h ^ (h >> 29))
}

/// `hash2d` mapped to `0..=1`.
///
/// The fraction comes from the **high** bits of the mixed hash — the part
/// splitmix64 avalanches best — so neighbouring lattice cells decorrelate fully.
/// (Truncating the low bits instead produces a statistically weak stream and makes
/// value noise over this lattice degenerate.)
#[inline]
pub fn hash2d_unit(x: i32, y: i32, salt: u64) -> f32 {
    // take the fraction from the TOP bits of the mixed hash: values land
    // uniformly in [1, 2) and subtracting 1.0 gives an exact 0..1 draw whose bits
    // are the very best-mixed bits of the hash. (Truncating the low 40 bits
    // instead would leave a statistically weak stream and ruin value noise.)
    let h = mix(hash2d(x, y, salt));
    f32::from_bits(0x3F80_0000 | ((h >> 41) as u32)) - 1.0
}

/// Convert world coordinates to a lattice cell of `cell` size.
#[inline]
pub fn world_to_cell(v: f32, cell: f32) -> i32 {
    if cell <= 0.0 {
        0
    } else {
        (v / cell).floor() as i32
    }
}
