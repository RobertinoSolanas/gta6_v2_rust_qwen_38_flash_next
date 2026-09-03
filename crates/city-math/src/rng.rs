//! Deterministic, reproducible pseudo random numbers (PCG-XSH-RR 32 bit).
//!
//! The whole application derives its world, its crowds and its material variation
//! from [`Rng`], so "same seed → same city" holds for native tests and for the
//! browser runtime tests.

use crate::Vec2;

/// A tiny PCG generator: 64 bit state, 32 bit output.
#[derive(Clone, Debug, PartialEq)]
pub struct Rng {
    state: u64,
    inc: u64,
}

const MULT: u64 = 6364_1362_2384_6793_005;

impl Rng {
    /// Create a generator from a seed. Seed `0` is valid.
    pub fn new(seed: u64) -> Rng {
        let mut r = Rng {
            state: 0,
            inc: 0x2360_cd3e_5f1c_ed1b | 1,
        };
        // Standard PCG warm-up.
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }

    /// Advance and return a raw `u32`.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULT).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform `u64` built from two `u32`s.
    pub fn next_u64(&mut self) -> u64 {
        ((self.next_u32() as u64) << 32) | (self.next_u32() as u64)
    }

    /// Uniform in `[0, 1)`.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        // 24 explicit mantissa bits: exact in f32, cheap, no denormals.
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }

    /// Uniform in `[min, max)`.
    #[inline]
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    /// Uniform integer in `[min, max]` (both inclusive).
    pub fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        let span = (max - min + 1) as u32;
        min + (self.next_u32() % span) as i32
    }

    /// Uniform index in `[0, n)`; returns `0` when `n == 0`.
    pub fn index(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u32() as usize) % n
    }

    /// `true` with probability `p`.
    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }

    /// Index of the first entry whose cumulative weight passes a draw.
    ///
    /// Weights need no normalisation; negative weights are treated as `0`.
    pub fn weighted(&mut self, weights: &[f32]) -> usize {
        let total: f32 = weights.iter().map(|w| w.max(0.0)).sum();
        if total <= 0.0 || weights.is_empty() {
            return 0;
        }
        let mut target = self.next_f32() * total;
        for (i, w) in weights.iter().enumerate() {
            target -= w.max(0.0);
            if target <= 0.0 {
                return i;
            }
        }
        weights.len() - 1
    }

    /// A unit direction on the ground plane.
    pub fn direction2(&mut self) -> Vec2 {
        Vec2::from_angle(self.next_f32() * crate::TAU)
    }

    /// Re-seed deterministically (stable derived streams).
    pub fn reseed(&mut self, seed: u64) {
        *self = Rng::new(seed);
    }

    /// Snapshot the state so a branch can be explored independently.
    pub fn fork(&self) -> Rng {
        self.clone()
    }
}

impl Default for Rng {
    fn default() -> Self {
        Rng::new(0x5eed_1234)
    }
}
