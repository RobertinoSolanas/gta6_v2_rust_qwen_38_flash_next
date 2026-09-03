//! # city-math
//!
//! Shared kernel for every bounded context in the project: small, deterministic,
//! allocation-free linear algebra, 2D/3D geometry helpers and a reproducible RNG.
//!
//! Design notes:
//! * Everything is [`Clone`]-able plain data with `#[repr(C)]` so it can be pushed
//!   straight into GPU buffers.
//! * Matrices are **column-major** (`m[col][row]`), matching WebGL's `uniformMatrix4fv`.
//! * All trigonometry/`rand` replacements are hand written so a given seed produces the
//!   same world on every platform that supports IEEE-754 `f32`.

#![forbid(unsafe_code)]

pub mod geo;
pub mod hash;
pub mod mat;
pub mod rng;
pub mod vec;

pub use geo::{Aabb2, Aabb3, Ray2, Seg2};
pub use hash::{hash12, hash2d, mix};
pub use mat::Mat4;
pub use rng::Rng;
pub use vec::{Vec2, Vec3, Vec4};

/// `PI` as `f32`.
pub const PI: f32 = core::f32::consts::PI;
/// Two pi, as `f32`.
pub const TAU: f32 = core::f32::consts::TAU;
/// Small epsilon used for geometric comparisons.
pub const EPS: f32 = 1e-4;

/// Clamp `x` into `[min, max]`.
#[inline]
pub fn clamp(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

/// Clamp to `0..=1`.
#[inline]
pub fn saturate(x: f32) -> f32 {
    clamp(x, 0.0, 1.0)
}

/// Linear interpolation between `a` and `b`.
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * saturate(t)
}

/// Hermite smoothstep between the two edges.
#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < f32::EPSILON {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = saturate((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

/// Smoother (Perlin/Quintic) smoothstep.
#[inline]
pub fn smootherstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = saturate((x - edge0) / (edge1 - edge0));
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Map `x` from `[i0,i1]` into `[o0,o1]`.
#[inline]
pub fn remap(x: f32, i0: f32, i1: f32, o0: f32, o1: f32) -> f32 {
    let t = if (i1 - i0).abs() < f32::EPSILON {
        0.0
    } else {
        (x - i0) / (i1 - i0)
    };
    o0 + (o1 - o0) * t
}

/// Wrap an angle into `(-PI, PI]` — important for yaw deltas and shortest-arc turns.
#[inline]
pub fn wrap_angle(a: f32) -> f32 {
    if !a.is_finite() {
        return 0.0;
    }
    // `2.0 * PI` (not TAU) keeps `2*PI` exactly on the wrap boundary in f32.
    let period = 2.0 * PI;
    let mut a = a % period;
    if a <= -PI {
        a += period;
    } else if a > PI {
        a -= period;
    }
    a
}

/// Interpolate two angles along the shortest arc.
#[inline]
pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    a + wrap_angle(b - a) * saturate(t)
}

/// Wrap a scalar into `[0, period)`.
#[inline]
pub fn wrap_period(x: f32, period: f32) -> f32 {
    if period <= 0.0 {
        return 0.0;
    }
    
    (x % period + period) % period
}

/// Frame-rate independent exponential smoothing factor.
///
/// `rate` is "how much of the remaining distance is closed per second".
#[inline]
pub fn damp(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    current + (target - current) * (1.0 - (-rate * dt).exp())
}

/// Move `current` towards `target` by at most `max_delta`.
#[inline]
pub fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    let d = target - current;
    if d.abs() <= max_delta {
        target
    } else {
        current + d.signum() * max_delta
    }
}

/// Degrees to radians.
#[inline]
pub fn to_rad(deg: f32) -> f32 {
    deg * (PI / 180.0)
}

/// Radians to degrees.
#[inline]
pub fn to_deg(rad: f32) -> f32 {
    rad * (180.0 / PI)
}

/// Signed signum that never returns `NaN`/zero for `x == 0`.
#[inline]
pub fn sign(x: f32) -> f32 {
    if x < 0.0 {
        -1.0
    } else {
        1.0
    }
}
