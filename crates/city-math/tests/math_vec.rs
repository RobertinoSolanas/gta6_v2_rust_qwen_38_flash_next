//! Vec2 / Vec3 / Vec4 behaviour.

use city_math::vec::{Vec2, Vec3, Vec4};
use city_math::{EPS, PI};

#[test]
fn vec2_arithmetic_and_length() {
    let a = Vec2::new(3.0, 4.0);
    assert!((a.len() - 5.0).abs() < 1e-5);
    assert!((a.len_sq() - 25.0).abs() < 1e-5);
    assert_eq!(a + Vec2::new(1.0, -1.0), Vec2::new(4.0, 3.0));
    assert_eq!(a - Vec2::new(1.0, 4.0), Vec2::new(2.0, 0.0));
    assert_eq!(a * 2.0, Vec2::new(6.0, 8.0));
    assert_eq!(-a, Vec2::new(-3.0, -4.0));
    assert!((a.dist(Vec2::ZERO) - 5.0).abs() < 1e-5);
}

#[test]
fn vec2_norm_is_safe_for_zero() {
    assert_eq!(Vec2::ZERO.norm(), Vec2::ZERO);
    let n = Vec2::new(0.0, 7.0).norm();
    assert!((n.y - 1.0).abs() < 1e-6);
    assert!((n.len() - 1.0).abs() < 1e-5);
}

#[test]
fn vec2_cross_perp_and_rot90() {
    assert!((Vec2::X.cross(Vec2::Y) - 1.0).abs() < 1e-6);
    assert!((Vec2::Y.cross(Vec2::X) + 1.0).abs() < 1e-6);
    assert_eq!(Vec2::X.perp(), Vec2::Y);
    assert_eq!(Vec2::X.rot90(1), Vec2::Y);
    assert_eq!(Vec2::X.rot90(2), -Vec2::X);
    assert_eq!(Vec2::X.rot90(-1), -Vec2::Y);
    assert_eq!(Vec2::X.rot90(0), Vec2::X);
}

#[test]
fn vec2_clamp_len_never_grows() {
    let v = Vec2::new(10.0, 0.0).clamp_len(2.0);
    assert!((v.len() - 2.0).abs() < 1e-4);
    let small = Vec2::new(0.5, 0.0).clamp_len(2.0);
    assert!((small.x - 0.5).abs() < 1e-6);
    assert_eq!(Vec2::ZERO.clamp_len(3.0), Vec2::ZERO);
}

#[test]
fn vec3_basis_and_cross() {
    let c = Vec3::X.cross(Vec3::Z);
    assert!((c.y + 1.0).abs() < 1e-6, "X x Z must be -Y, got {c:?}");
    assert!(Vec3::UP.dot(Vec3::X).abs() < 1e-6);
    assert!((Vec3::new(1.0, 2.0, 2.0).len() - 3.0).abs() < 1e-6);
    assert_eq!(Vec3::new(2.0, 3.0, 4.0).with_y(0.0).y, 0.0);
}

#[test]
fn vec3_projection_to_ground() {
    let p = Vec3::new(3.0, 9.0, -4.0);
    assert_eq!(p.xz(), Vec2::new(3.0, -4.0));
    assert_eq!(Vec3::from_xz(Vec2::new(1.0, 2.0), 7.0), Vec3::new(1.0, 7.0, 2.0));
    assert_eq!(p.as_array(), [3.0, 9.0, -4.0]);
}

#[test]
fn vec3_yaw_matches_look_convention() {
    assert!(Vec3::Z.yaw().abs() < 1e-6, "+Z is yaw 0");
    assert!((Vec3::X.yaw() - PI / 2.0).abs() < 1e-6);
    assert!((Vec3::new(0.0, 0.0, -1.0).yaw() - PI).abs() < 1e-6);
}

#[test]
fn from_yaw_pitch_matches_look_convention() {
    let fwd = Vec3::from_yaw_pitch(0.0, 0.0);
    assert!((fwd.z - 1.0).abs() < 1e-5 && fwd.x.abs() < 1e-5);
    let up = Vec3::from_yaw_pitch(0.0, PI / 2.0);
    assert!((up.y - 1.0).abs() < 1e-5);
    let right = Vec3::from_yaw_pitch(PI / 2.0, 0.0);
    assert!((right.x - 1.0).abs() < 1e-5);
}

#[test]
fn vec3_min_max_and_assign_ops() {
    let a = Vec3::new(1.0, 5.0, -2.0);
    let b = Vec3::new(3.0, -1.0, 3.0);
    assert_eq!(a.min(b), Vec3::new(1.0, -1.0, -2.0));
    assert_eq!(a.max(b), Vec3::new(3.0, 5.0, 3.0));
    let mut m = a;
    m += Vec3::new(0.0, 1.0, 0.0);
    assert!((m.y - 6.0).abs() < 1e-6);
    m *= 2.0;
    assert!((m.x - 2.0).abs() < 1e-6 && (m.y - 12.0).abs() < 1e-6);
}

#[test]
fn vec3_clamp_len_and_lerp() {
    let v = Vec3::new(0.0, 0.0, 50.0).clamp_len(1.0);
    assert!((v.len() - 1.0).abs() < 1e-4);
    assert_eq!(Vec3::ZERO.lerp(Vec3::new(4.0, 8.0, 2.0), 0.5), Vec3::new(2.0, 4.0, 1.0));
    assert_eq!(Vec3::ZERO.clamp_len(2.0), Vec3::ZERO);
}

#[test]
fn vec4_colour_helpers() {
    let c = Vec4::rgb(0.5, 0.25, 1.0);
    assert_eq!(c.w, 1.0);
    let dark = c.lit(0.1);
    assert!(c.x > dark.x && dark.y > 0.0);
    let clipped = c.lit(100.0);
    assert!(clipped.x <= 1.0 && clipped.z <= 1.0);
    assert_eq!(Vec4::ONE.to_array(), [1.0, 1.0, 1.0, 1.0]);
    assert!((Vec4::rgb(1.0, 1.0, 1.0).luminance() - 1.0).abs() < 1e-5);
    assert_eq!((Vec4::rgb(1.0, 0.0, 0.0) * 2.0).x, 2.0);
    assert!((Vec4::rgb(0.5, 0.0, 0.0) + Vec4::rgb(0.25, 0.0, 0.0)).x - 0.75 < 1e-6);
}

#[test]
fn scalar_utilities_behave() {
    assert_eq!(city_math::clamp(5.0, 0.0, 3.0), 3.0);
    assert_eq!(city_math::saturate(-1.0), 0.0);
    assert_eq!(city_math::lerp(0.0, 10.0, 0.25), 2.5);
    assert!((city_math::smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
    assert!((city_math::smootherstep(0.0, 1.0, 0.0)).abs() < 1e-6);
    assert!((city_math::remap(5.0, 0.0, 10.0, 100.0, 200.0) - 150.0).abs() < 1e-4);
    assert_eq!(city_math::sign(-3.0), -1.0);
    assert_eq!(city_math::sign(0.0), 1.0);
    assert_eq!(city_math::move_towards(0.0, 10.0, 3.0), 3.0);
    assert_eq!(city_math::move_towards(2.0, 1.0, 3.0), 1.0);
    assert!(EPS > 0.0 && EPS < 1e-3);
    assert!(1.0 + EPS != 1.0);
}

#[test]
fn angle_wrapping_is_symmetric() {
    assert!(city_math::wrap_angle(0.0).abs() < 1e-6);
    // A full turn must land back on zero (regression test for TAU rounding).
    assert!(city_math::wrap_angle(std::f32::consts::TAU).abs() < 1e-6);
    assert!(city_math::wrap_angle(-std::f32::consts::TAU).abs() < 1e-6);
    assert!(city_math::wrap_angle(100.0 * std::f32::consts::TAU).abs() < 1e-3);
    // Just past PI folds to the other side.
    assert!((city_math::wrap_angle(PI + 0.001) - (-PI + 0.001)).abs() < 1e-3);
    assert!((city_math::wrap_angle(-PI - 0.001) - (PI - 0.001)).abs() < 1e-3);
    // Multiple turns.
    assert!((city_math::wrap_angle(PI * 2.5) - PI / 2.0).abs() < 1e-4);
    assert!((city_math::wrap_angle(-PI * 2.5) + PI / 2.0).abs() < 1e-4);
    // Values already in range are untouched.
    assert!((city_math::wrap_angle(0.75) - 0.75).abs() < 1e-6);
    assert!(!city_math::wrap_angle(f32::NAN).is_nan(), "NaN input must not spread");
    // shortest arc from 170deg to -170deg goes through 180, not through 0
    let a = city_math::to_rad(170.0);
    let b = city_math::to_rad(-170.0);
    let m = city_math::lerp_angle(a, b, 0.5);
    assert!(
        (city_math::wrap_angle(m) - PI).abs() < 0.05,
        "shortest arc must pass +-PI, got {m}"
    );
    // A quarter turn the short way.
    let q = city_math::lerp_angle(0.0, PI / 2.0, 0.5);
    assert!((q - PI / 4.0).abs() < 1e-4);
    assert_eq!(city_math::lerp_angle(0.0, 1.0, 0.0), 0.0);
    assert!((city_math::lerp_angle(0.3, 0.4, 1.0) - 0.4).abs() < 1e-6);
}

#[test]
fn wrap_period_stays_in_range() {
    for v in [-12.5f32, -0.1, 0.0, 0.3, 7.9, 100.0] {
        let w = city_math::wrap_period(v, 24.0);
        assert!(v >= 0.0 && (0.0..24.0).contains(&w) || v < 0.0 && (0.0..24.0).contains(&w), "v={v} w={w}");
    }
    assert_eq!(city_math::wrap_period(1.0, 0.0), 0.0);
}

#[test]
fn damp_converges_monotonically() {
    let mut x = 0.0f32;
    let mut prev = -1.0f32;
    for _ in 0..600 {
        x = city_math::damp(x, 10.0, 8.0, 1.0 / 60.0);
        assert!(x >= 0.0 && x <= 10.0);
        assert!(x >= prev);
        prev = x;
    }
    assert!((x - 10.0).abs() < 0.01);
}
