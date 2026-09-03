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
/// stable per-lot variations.
#[inline]
pub fn hash2d(x: i32, y: i32, salt: u64) -> u64 {
    hash12(((x as i64) as u64).wrapping_mul(0x1000_0000_1b3), (y as u64) ^ mix(salt))
}

/// `hash2d` mapped to `0..=1`.
#[inline]
pub fn hash2d_unit(x: i32, y: i32, salt: u64) -> f32 {
    (hash2d(x, y, salt) >> 40) as f32 / (1u64 << 24) as f32
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
