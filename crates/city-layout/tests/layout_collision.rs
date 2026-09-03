//! Collision: walkability queries and the push-out resolution used by the avatar.

use city_layout::{City, CityParams, SidewalkLoop};
use city_math::{Aabb2, Vec2};

const R: f32 = 0.4;

fn default_city() -> City {
    City::generate(CityParams::default())
}

/// A building with a footprint big enough to have a real interior.
fn big_building(city: &City) -> Aabb2 {
    city.buildings()
        .iter()
        .map(|b| b.footprint)
        .find(|f| f.size().x > 8.0 && f.size().y > 8.0)
        .expect("a building with an interior")
}

/// A waypoint on a sidewalk loop that is free of solid geometry.
fn open_spot(city: &City) -> Vec2 {
    city.loops()
        .iter()
        .flat_map(|l: &SidewalkLoop| l.points().iter().copied())
        .find(|p| city.is_walkable(*p, R))
        .expect("some waypoint is walkable")
}

#[test]
fn spawn_is_walkable() {
    let city = default_city();
    assert!(city.is_walkable(city.spawn_point(), R));
    assert!(city.is_walkable(city.spawn_point(), 1.0));
}

#[test]
fn building_interiors_are_not_walkable() {
    let city = default_city();
    let interior = big_building(&city).center();
    assert!(
        !city.is_walkable(interior, R),
        "buildings must block movement"
    );
    assert!(!city.buildings_at(interior).is_empty());
}

#[test]
fn radius_matters_for_walkability() {
    let city = default_city();
    let spot = open_spot(&city);
    // A tiny agent fits; a huge one runs into the surrounding furniture.
    assert!(city.is_walkable(spot, 0.05));
    assert!(!city.is_walkable(big_building(&city).center(), 0.05));
}

#[test]
fn resolve_pushes_out_of_a_building() {
    let city = default_city();
    let footprint = big_building(&city);
    let inside = footprint.center();
    let out = city.resolve(inside, R);
    assert_ne!(out, inside, "resolve did nothing");
    // The correction must be substantial, not a nudge.
    assert!(
        out.dist(inside) > 0.1,
        "correction too small: {:?}",
        out - inside
    );
}

#[test]
fn resolve_leaves_free_ground_alone() {
    let city = default_city();
    let spot = open_spot(&city);
    let fixed = city.resolve(spot, R);
    assert!(spot.dist(fixed) < 0.75, "{spot:?} was pushed to {fixed:?}");
}

#[test]
fn resolve_clamps_to_the_city() {
    let city = default_city();
    let bounds = city.bounds();
    for p in [Vec2::new(-500.0, -500.0), Vec2::new(100_000.0, 42.0)] {
        let fixed = city.resolve(p, R);
        assert!(
            fixed.x >= bounds.min.x && fixed.x <= bounds.max.x,
            "x escaped: {fixed:?}"
        );
        assert!(fixed.y >= bounds.min.y && fixed.y <= bounds.max.y);
    }
}

#[test]
fn most_sidewalk_ground_is_walkable() {
    let city = default_city();
    for block in city.blocks() {
        let ring = &city.loops()[block.loop_index];
        let pts: Vec<Vec2> = ring.points().to_vec();
        let free = pts.iter().filter(|p| city.is_walkable(**p, R)).count();
        // Solid street furniture may block a few waypoints, but the loop as a whole
        // has to stay walkable, otherwise the crowd would be trapped.
        assert!(
            free * 2 >= pts.len(),
            "block {:?}: only {}/{} waypoints walkable",
            block.cell,
            free,
            pts.len()
        );
    }
}

#[test]
fn carriageways_are_not_solid() {
    // Jaywalking must be possible: the tarmac is never registered as an obstacle.
    let city = default_city();
    let params = city.params().clone();
    for road in city.roads().iter().take(10) {
        let line = road.center_line(&params);
        let mid = line.a.lerp(line.b, 0.5);
        assert!(
            city.is_walkable(mid, 0.2),
            "road {} should not block walking",
            road.id
        );
    }
}

#[test]
fn distance_to_road_is_zero_on_tarmac() {
    let city = default_city();
    let params = city.params().clone();
    let road = city.roads().first().expect("a road");
    let line = road.center_line(&params);
    let mid = line.a.lerp(line.b, 0.5);
    assert!(city.distance_to_road(mid) < 0.01);
    // A point far off the grid measures a positive distance.
    let corner = city.bounds().min - Vec2::new(2.0, 2.0);
    assert!(city.distance_to_road(corner) > 0.0);
}
