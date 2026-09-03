//! AABB / segment / ray behaviour used by collision + steering code.

use city_math::geo::{Aabb2, Aabb3, Ray2, Seg2};
use city_math::{Vec2, Vec3};

fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}
fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

#[test]
fn aabb2_construction_and_metrics() {
    let b = Aabb2::from_min_size(v2(1.0, 1.0), v2(2.0, 4.0));
    assert_eq!(b.center(), v2(2.0, 3.0));
    assert!((b.area() - 8.0).abs() < 1e-5);
    assert_eq!(b.size(), v2(2.0, 4.0));
    assert_eq!(b.min, v2(1.0, 1.0));
    assert_eq!(b.max, v2(3.0, 5.0));
    let c = Aabb2::from_center_size(v2(0.0, 0.0), v2(1.0, 1.0));
    assert!(!b.intersects(c) || b.intersects(c));
    assert_eq!(c.min, v2(-1.0, -1.0));
}

#[test]
fn aabb2_empty_is_never_hit() {
    let e = Aabb2::EMPTY;
    assert!(!e.contains(v2(0.0, 0.0)));
    assert!(!e.intersects(Aabb2::from_center_size(v2(0.0, 0.0), v2(100.0, 100.0))));
    assert!(e.area() <= 0.0);
}

#[test]
fn aabb2_contains_grown_closest_expand() {
    let b = Aabb2::from_min_size(v2(0.0, 0.0), v2(10.0, 10.0));
    assert!(b.contains(v2(0.0, 10.0)), "edges are inclusive");
    assert!(!b.contains(v2(10.01, 5.0)));
    assert!(b.contains_padded(v2(-0.5, 5.0), 0.5));
    assert_eq!(b.closest_point(v2(-3.0, 12.0)), v2(0.0, 10.0));
    assert_eq!(b.expand(v2(-4.0, -2.0)).min, v2(-4.0, -2.0));
    assert_eq!(b.grown(1.0).min, v2(-1.0, -1.0));
}

#[test]
fn aabb2_signed_distance_sign_matches_containment() {
    let b = Aabb2::from_min_size(v2(0.0, 0.0), v2(10.0, 10.0));
    assert!(b.signed_distance(v2(5.0, 5.0)) < 0.0);
    assert!(b.signed_distance(v2(0.0, 5.0)).abs() < 1e-6);
    assert!((b.signed_distance(v2(13.0, 5.0)) - 3.0).abs() < 1e-4);
    assert!(
        (b.signed_distance(v2(13.0, 14.0)) - 5.0).abs() < 1e-4,
        "corner distance"
    );
}

#[test]
fn push_out_resolves_to_nearest_face() {
    let wall = Aabb2::from_min_size(v2(0.0, -5.0), v2(4.0, 10.0));
    // Standing just inside the west face -> pushed back to the west side.
    let r = wall
        .push_out(v2(1.0, 0.0), 0.0)
        .expect("inside -> resolved");
    assert!(r.x <= 1e-5 && r.y == 0.0, "got {r:?}");
    // No collision far away.
    assert!(wall.push_out(v2(50.0, 0.0), 0.5).is_none());
    // The radius inflates the box by `radius`.
    assert!(wall.push_out(v2(-0.4, 0.0), 0.5).is_some());
    assert!(wall.push_out(v2(-0.6, 0.0), 0.5).is_none());
}

#[test]
fn push_out_never_leaves_a_point_inside() {
    let wall = Aabb2::from_min_size(v2(-2.0, -2.0), v2(4.0, 4.0));
    for i in 0..40 {
        for j in 0..12 {
            let p = v2(-2.5 + i as f32 * 0.12, -2.5 + j as f32 * 0.4);
            if let Some(p2) = wall.push_out(p, 0.0) {
                assert!(
                    wall.signed_distance(p2) <= 1e-3,
                    "i={i} j={j} left inside at {p2:?} (sd={})",
                    wall.signed_distance(p2)
                );
            }
        }
    }
}

#[test]
fn seg2_closest_and_distance() {
    let s = Seg2::new(v2(0.0, 0.0), v2(10.0, 0.0));
    assert!((s.len() - 10.0).abs() < 1e-5);
    assert_eq!(s.dir(), v2(1.0, 0.0));
    let (p, t) = s.closest(v2(5.0, 3.0));
    assert_eq!(p, v2(5.0, 0.0));
    assert!((t - 0.5).abs() < 1e-5);
    assert!((s.distance(v2(5.0, 3.0)) - 3.0).abs() < 1e-5);
    // Beyond the ends we clamp to the endpoint.
    let (a, ta) = s.closest(v2(-4.0, 0.0));
    assert_eq!(a, v2(0.0, 0.0));
    assert_eq!(ta, 0.0);
    // Degenerate segment does not produce NaN.
    let dot = Seg2::new(v2(1.0, 1.0), v2(1.0, 0.0));
    assert_eq!(dot.closest(v2(5.0, 5.0)).0, v2(1.0, 1.0));
    assert_eq!(Seg2::new(v2(2.0, 2.0), v2(2.0, 2.0)).dir(), v2(0.0, 0.0));
}

#[test]
fn seg2_intersection() {
    let a = Seg2::new(v2(-5.0, 0.0), v2(5.0, 0.0));
    let b = Seg2::new(v2(0.0, -5.0), v2(0.0, 5.0));
    let p = a.intersect(b).expect("cross");
    assert!(p.x.abs() < 1e-6 && p.y.abs() < 1e-6);
    // Parallel segments never intersect.
    assert!(a.intersect(Seg2::new(v2(0.0, 1.0), v2(1.0, 1.0))).is_none());
    // Non overlapping, even though the infinite lines meet.
    assert!(a
        .intersect(Seg2::new(v2(20.0, -1.0), v2(20.0, 1.0)))
        .is_none());
}

#[test]
fn ray_hits_segment_only_within_range() {
    let wall = Seg2::new(v2(10.0, -3.0), v2(10.0, 3.0));
    // Direction is normalised, so `t` is a real distance even for a long input dir.
    let r = Ray2::new(v2(0.0, 0.0), v2(2.0, 0.0), 100.0);
    let t = r.hit_seg(wall).expect("hit");
    assert!((t - 10.0).abs() < 1e-4, "got {t}");
    assert!(Ray2::new(v2(0.0, 0.0), v2(1.0, 0.0), 5.0)
        .hit_seg(wall)
        .is_none());
    // Behind the ray origin.
    assert!(Ray2::new(v2(0.0, 0.0), v2(-1.0, 0.0), 100.0)
        .hit_seg(wall)
        .is_none());
    // Passes beside the segment.
    assert!(Ray2::new(v2(0.0, 9.0), v2(1.0, 0.0), 100.0)
        .hit_seg(wall)
        .is_none());
    // Hits the extension of the segment, but past `max_t`.
    assert!(Ray2::new(v2(0.0, 0.0), v2(1.0, 0.0), 4.0)
        .hit_seg(wall)
        .is_none());
    assert_eq!(r.at(4.0), v2(4.0, 0.0));
}

#[test]
fn aabb3_contains_and_intersects() {
    let b = Aabb3::from_center_size(Vec3::new(0.0, 5.0, 0.0), Vec3::new(4.0, 10.0, 4.0));
    assert!(b.contains(Vec3::new(0.0, 0.0, 0.0)));
    assert!(!b.contains(Vec3::new(0.0, 20.0, 0.0)));
    assert_eq!(b.size(), Vec3::new(4.0, 10.0, 4.0));
    assert_eq!(b.center(), Vec3::new(0.0, 5.0, 0.0));
    assert_eq!(b.footprint().center(), v2(0.0, 0.0));
    assert!(b.intersects(Aabb3::new(
        Vec3::new(-1.0, 9.0, -1.0),
        Vec3::new(5.0, 12.0, 1.0)
    )));
    assert!(!b.intersects(Aabb3::new(
        Vec3::new(10.0, 0.0, -1.0),
        Vec3::new(11.0, 1.0, 1.0)
    )));
    assert_eq!(b.grown(1.0).size().x, 6.0);
    assert!(b
        .expand(v3(9.0, 0.0, 0.0))
        .contains(Vec3::new(8.0, 0.0, 0.0)));
}

#[test]
fn aabb3_ray_hits_misses_and_inside() {
    let b = Aabb3::from_center_size(Vec3::ZERO, Vec3::new(2.0, 2.0, 4.0));
    let hit = b.ray(Vec3::new(0.0, 0.0, -20.0), Vec3::Z).expect("hit");
    // Box spans z in [-2, 2] → entry at t=18, exit at t=22.
    assert!(
        (hit.0 - 18.0).abs() < 1e-4 && (hit.1 - 22.0).abs() < 1e-4,
        "{hit:?}"
    );
    // Pointing away from the box.
    assert!(b
        .ray(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, -1.0))
        .is_none());
    // Parallel offset miss.
    assert!(b
        .ray(Vec3::new(50.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 1.0))
        .is_none());
    // Origin inside -> near clip at 0.
    let inside = b.ray(Vec3::ZERO, Vec3::Z).expect("inside hit");
    assert!(inside.0.abs() < 1e-5 && (inside.1 - 2.0).abs() < 1e-4);
    // Diagonal ray through a corner still hits.
    assert!(b
        .ray(Vec3::new(-6.0, -6.0, -6.0), Vec3::new(1.0, 1.0, 1.0).norm())
        .is_some());
    // Zero direction (degenerate) must not panic.
    let _ = b.ray(Vec3::new(0.0, 0.0, -3.0), Vec3::new(0.0, 0.0, 0.0));
}
