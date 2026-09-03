//! Mat4 construction, products and projections.

use city_math::vec::{Vec3, Vec4};
use city_math::{Mat4, PI};

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn identity_is_neutral() {
    let m = Mat4::IDENTITY;
    let p = Vec3::new(1.0, 2.0, 3.0);
    assert_eq!(m.point(p), p);
    assert_eq!(m.dir(p), p);
    assert_eq!(m.mul(&Mat4::IDENTITY), Mat4::IDENTITY);
    assert!(approx(m.avg_scale(), 1.0));
}

#[test]
fn translation_moves_points_only() {
    let t = Mat4::translation(Vec3::new(10.0, -2.0, 4.0));
    assert_eq!(t.point(Vec3::new(1.0, 1.0, 1.0)), Vec3::new(11.0, -1.0, 5.0));
    // directions ignore translation
    assert_eq!(t.dir(Vec3::X), Vec3::X);
    assert!(approx(t.at(3, 0), 10.0));
    assert!(approx(t.at(3, 3), 1.0));
}

#[test]
fn scale_applies_per_axis() {
    let s = Mat4::scale(Vec3::new(2.0, 3.0, 4.0));
    assert_eq!(s.point(Vec3::new(1.0, 1.0, 1.0)), Vec3::new(2.0, 3.0, 4.0));
    assert!(approx(Mat4::scale_uniform(3.0).avg_scale(), 3.0));
}

#[test]
fn yaw_rotation_follows_right_hand_rule() {
    let r = Mat4::rotate_y(PI / 2.0);
    let v = r.dir(Vec3::Z);
    assert!(approx(v.x, 1.0) && v.z.abs() < 1e-4, "+Z yawed 90deg must be +X, got {v:?}");
    let back = Mat4::rotate_y(-PI / 2.0).dir(Vec3::Z);
    assert!(approx(back.x, -1.0));
    // yaw is preserved through compose()
    let c = Mat4::compose(Vec3::ZERO, PI / 2.0, 0.0, Vec3::new(1.0, 1.0, 1.0));
    let v2 = c.dir(Vec3::Z);
    assert!(approx(v2.x, 1.0) && v2.z.abs() < 1e-4, "got {v2:?}");
}

#[test]
fn pitch_rotates_around_local_x() {
    let r = Mat4::rotate_x(PI / 2.0);
    let v = r.dir(Vec3::Z);
    assert!(approx(v.y.abs(), 1.0), "pitch 90deg must align Z with Y, got {v:?}");
    assert!(approx(r.dir(Vec3::X).x, 1.0), "X axis is the pitch pivot");
}

#[test]
fn compose_matches_manual_rotation_chain() {
    let t = Vec3::new(3.0, 4.0, 5.0);
    let (yaw, pitch) = (0.7, -0.35);
    let scale = Vec3::new(2.0, 0.5, 1.5);
    let composed = Mat4::compose(t, yaw, pitch, scale);
    let chained = Mat4::translation(t)
        .mul(&Mat4::rotate_y(yaw))
        .mul(&Mat4::rotate_z(pitch))
        .mul(&Mat4::scale(scale));
    // The composed matrix must place the local origin at `translation` ...
    let origin = composed.point(Vec3::ZERO);
    assert!(
        (origin - t).len() < 1e-3,
        "local origin must land on translation, got {origin:?}"
    );
    // ... and scale a local offset before rotating it.
    let local_up = composed.point(Vec3::new(0.0, 1.0, 0.0)) - composed.point(Vec3::ZERO);
    assert!(
        (local_up.y - scale.y * pitch.cos()).abs() < 1e-3,
        "scale must be applied before pitch: {local_up:?}"
    );
    assert!((local_up.len() - scale.y).abs() < 1e-3, "length preserved by rotation");
    for c in 0..4 {
        for r in 0..4 {
            assert!(
                approx(composed.at(c, r), chained.at(c, r)),
                "mismatch at col {c} row {r}: {} vs {}",
                composed.at(c, r),
                chained.at(c, r)
            );
        }
    }
    // The composed matrix places the local origin at `translation`.
    assert!(approx(composed.at(3, 0), t.x));
    assert!(approx(composed.at(3, 1), t.y));
}

#[test]
fn matrix_product_applies_right_first() {
    let t = Mat4::translation(Vec3::new(1.0, 0.0, 0.0));
    let s = Mat4::scale_uniform(2.0);
    // scale then translate
    let m = t.mul(&s);
    assert_eq!(m.point(Vec3::X), Vec3::new(3.0, 0.0, 0.0));
    // translate then scale
    let m2 = s.mul(&t);
    assert_eq!(m2.point(Vec3::X), Vec3::new(4.0, 0.0, 0.0));
}

#[test]
fn perspective_maps_view_depth_to_ndc() {
    let (n, f) = (0.1f32, 500.0f32);
    let p = Mat4::perspective(60f32.to_radians(), 16.0 / 9.0, 0.1, 500.0);
    let ndc = |z: f32| {
        let c = p.vec4(Vec4::new(0.0, 0.0, z, 1.0));
        c.z / c.w
    };
    // OpenGL convention: view space looks down -Z, near -> -1, far -> +1.
    assert!((ndc(-n) + 1.0).abs() < 1e-4, "near plane: {}", ndc(-n));
    assert!((ndc(-f) - 1.0).abs() < 1e-4, "far plane: {}", ndc(-f));
    let mut prev = -1.0f32;
    for i in 1..40 {
        let z = -n - (f - n) * (i as f32 / 40.0);
        let v = ndc(z);
        assert!(v > prev && v <= 1.0 + 1e-5, "depth must increase monotonically: {prev} -> {v}");
        prev = v;
    }
    let behind = p.vec4(Vec4::new(0.0, 0.0, 10.0, 1.0));
    assert!(behind.w < 0.0, "points behind the camera have w < 0");
}

#[test]
fn perspective_respects_fov_and_aspect() {
    let wide = Mat4::perspective(90f32.to_radians(), 1.0, 0.1, 100.0);
    let narrow = Mat4::perspective(30f32.to_radians(), 1.0, 0.1, 100.0);
    assert!(wide.at(0, 0) < narrow.at(0, 0), "wider fov → smaller x scale");
    let square = Mat4::perspective(60f32.to_radians(), 2.0, 0.1, 100.0);
    assert!(square.at(0, 0) < narrow.at(0, 0), "wider aspect shrinks x scale");
    assert!(square.at(1, 1) > 1.0);
}

#[test] 
fn ortho_maps_volume_to_unit_cube() {
    let o = Mat4::ortho(-10.0, 10.0, -5.0, 10.0, 0.1, 100.0);
    // View space depth is negative, just like for the perspective matrix.
    let near_corner = o.point(Vec3::new(-10.0, -5.0, -0.1));
    assert!(
        approx(near_corner.x, -1.0) && approx(near_corner.y, -1.0) && approx(near_corner.z, -1.0),
        "{near_corner:?}"
    );
    let far_corner = o.point(Vec3::new(10.0, 10.0, -100.0));
    assert!(
        approx(near_corner.z + near_corner.x + 2.0, 0.0)
            && approx(far_corner.x, 1.0)
            && approx(far_corner.y, 1.0)
            && approx(far_corner.z, 1.0),
        "{far_corner:?}"
    );
    // The centre of the view volume maps to the origin of NDC.
    let mid = o.point(Vec3::new(0.0, 2.5, -50.05));
    assert!(
        approx(mid.x, 0.0) && approx(mid.y, 0.0) && approx(mid.z, 0.0),
        "{mid:?}"
    );
}

#[test]
fn look_at_places_eye_at_origin() {
    let view = Mat4::look_at(Vec3::new(0.0, 5.0, 10.0), Vec3::ZERO, Vec3::UP);
    let eye = view.point(Vec3::new(0.0, 5.0, 10.0));
    assert!(eye.len() < 1e-3, "eye must map to origin, got {eye:?}");
    let target = view.point(Vec3::ZERO);
    assert!(target.z < 0.0, "target must be in front of the camera (negative z)");
    assert!(target.y.abs() < 0.2);
}

#[test]
fn look_at_handles_degenerate_up_vector() {
    let v = Mat4::look_at(Vec3::ZERO, Vec3::UP, Vec3::UP);
    let p = v.point(Vec3::new(1.0, 1.0, 1.0));
    assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite(), "{p:?}");
}

#[test]
fn flatten_is_column_major() {
    let t = Mat4::translation(Vec3::new(7.0, 8.0, 9.0));
    let f = t.to_flat();
    assert_eq!(f.len(), 16);
    assert_eq!(&f[0..4], &[1.0, 0.0, 0.0, 0.0]);
    assert_eq!(&f[12..16], &[7.0, 8.0, 9.0, 1.0]);
    // Row major: the translation lives in the fourth *row*, column major keeps it
    // in the last column — the layout WebGL expects.
    let r = t.row_major();
    assert_eq!(r[0], [1.0, 0.0, 0.0, 7.0]);
    assert_eq!(r[3], [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(Mat4::IDENTITY.to_flat(), [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0
    ]);
}

#[test]
fn default_is_identity() {
    assert_eq!(Mat4::default(), Mat4::IDENTITY);
    assert_eq!(Mat4::from_cols(Mat4::IDENTITY.cols), Mat4::IDENTITY);
}

#[test]
fn bias_shifts_only_translation() {
    let a = Mat4::IDENTITY.bias_xz(0.01, -0.02);
    assert!(approx(a.at(3, 0), 0.01) && approx(Mat4::IDENTITY.at(3, 0), 0.0));
    assert!(approx(a.at(0, 0), 1.0));
}

#[test]
fn rad_deg_helpers() {
    assert!((city_math::to_rad(180.0) - PI).abs() < 1e-6);
    assert!((city_math::to_deg(PI) - 180.0).abs() < 1e-4);
    assert!((city_math::to_deg(city_math::to_rad(37.5)) - 37.5).abs() < 1e-4);
}
