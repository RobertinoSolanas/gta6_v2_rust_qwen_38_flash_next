//! Broad-phase index: bucketing, point/circle queries and the determinism checksum.

use city_layout::{IndexItem, IndexKind, SpatialIndex, CELL_SIZE};
use city_math::{Aabb2, Vec2};

/// A square footprint centred on `center`, `half` metres to each side.
fn solid(center: Vec2, half: f32, height: f32) -> IndexItem {
    IndexItem {
        id: 0,
        kind: IndexKind::Building,
        solid: Aabb2::new(
            Vec2::new(center.x - half, center.y - half),
            Vec2::new(center.x + half, center.y + half),
        ),
        height,
    }
}

fn at(x: f32, y: f32) -> IndexItem {
    solid(Vec2::new(x, y), 2.0, 9.0)
}

fn bounds() -> Aabb2 {
    Aabb2::from_min_size(Vec2::ZERO, Vec2::new(240.0, 240.0))
}

fn build_many() -> SpatialIndex {
    let mut index = SpatialIndex::new(CELL_SIZE, bounds());
    for i in 0..40 {
        let p = Vec2::new(20.0 + i as f32 * 5.0, 20.0 + (i % 7) as f32 * 9.0);
        index.insert(solid(p, 2.0, 6.0 + i as f32));
    }
    index
}

#[test]
fn empty_index_has_no_items() {
    let index = SpatialIndex::new(CELL_SIZE, bounds());
    assert!(index.is_empty());
    assert_eq!(index.len(), 0);
    assert!(!index.overlaps_circle(Vec2::new(10.0, 10.0), 1.0));
    assert!(!index.contains_point(Vec2::new(10.0, 10.0)));
    assert_eq!(index.height_at(Vec2::new(10.0, 10.0)), 0.0);
    assert!(index.grid_dims()[0] > 0 && index.grid_dims()[1] > 0);
}

#[test]
fn insert_then_query() {
    let mut index = SpatialIndex::new(CELL_SIZE, bounds());
    index.insert(solid(Vec2::new(50.0, 50.0), 3.0, 12.0));
    assert_eq!(index.len(), 1);
    assert!(index.contains_point(Vec2::new(50.0, 50.0)));
    assert!(index.overlaps_circle(Vec2::new(50.0, 50.0), 1.0));
    assert_eq!(index.height_at(Vec2::new(50.0, 50.0)), 12.0);
    assert!(!index.contains_point(Vec2::new(80.0, 80.0)));
    assert_eq!(index.height_at(Vec2::new(80.0, 80.0)), 0.0);
    assert!(index.item(0).is_some());
    assert!(index.item(1).is_none());
}

#[test]
fn overlaps_only_near_solids() {
    let mut index = SpatialIndex::new(CELL_SIZE, bounds());
    index.insert(at(30.0, 60.0));
    index.insert(at(31.0, 45.0));
    index.insert(at(200.0, 200.0));
    assert_eq!(index.len(), 3);
    assert!(index.overlaps_circle(Vec2::new(30.0, 45.0), 4.0));
    assert!(!index.overlaps_circle(Vec2::new(120.0, 30.0), 2.0));
}

#[test]
fn candidates_are_never_duplicated() {
    let index = build_many();
    // A wide query touches many cells; an item registered in several of them must
    // still be reported once.
    let c = index.candidates(Vec2::new(60.0, 40.0), 25.0);
    assert!(!c.is_empty());
    let mut sorted = c.clone();
    sorted.sort_unstable();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(sorted.len(), before, "duplicate candidate ids");
}

#[test]
fn nearest_reports_distance_and_item() {
    let mut index = SpatialIndex::new(CELL_SIZE, bounds());
    index.insert(solid(Vec2::new(60.0, 60.0), 2.0, 20.0));
    let (d, item) = index.nearest(Vec2::new(40.0, 60.0), 30.0).expect("hit");
    assert!((d - 18.0).abs() < 1e-3, "distance {d}");
    assert_eq!(item.height, 20.0);
    // Nothing within a tight radius far away.
    assert!(index.nearest(Vec2::new(220.0, 40.0), 1.0).is_none());
}

#[test]
fn identical_builds_share_a_checksum() {
    let a = build_many();
    let b = build_many();
    assert_eq!(a.len(), 40);
    assert_eq!(a.checksum(), b.checksum());
    assert_eq!(a.cell_size(), CELL_SIZE);
}

#[test]
fn checksum_changes_when_geometry_moves() {
    let base = build_many();
    let mut moved = SpatialIndex::new(CELL_SIZE, bounds());
    for it in base.items() {
        let mut shifted = it.clone();
        shifted.solid = shifted.solid.grown(0.5);
        moved.insert(shifted);
    }
    assert_eq!(base.len(), moved.len());
    assert_ne!(base.checksum(), moved.checksum());
}
