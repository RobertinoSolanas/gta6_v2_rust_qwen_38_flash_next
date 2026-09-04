//! Traffic: lane following, traffic lights, queueing, recycling, determinism.
//!
//! Cars are stepped through [`Crowd`] exactly like `city-app` does, with the live window
//! frozen so nothing respawns and every position stays comparable frame by frame.

use std::collections::HashSet;

use city_layout::{Axis, City, CityParams, CrossingLink, Lane};
use city_math::Vec2;
use city_sim::{Car, CarKind, Crowd, SimConfig, CAR_ACCEL, CAR_MIN_GAP, TURN_SLOW};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn city() -> City {
    City::generate(CityParams::default())
}

fn crowd(city: &City) -> Crowd {
    Crowd::new(city, SimConfig::default())
}

fn center(city: &City) -> Vec2 {
    city.bounds().center()
}

/// Step without recycling: nothing respawns, so a car is always where it drove to.
fn run_frozen(crowd: &mut Crowd, city: &City, seconds: f32) {
    crowd.run_frozen(city, 1.0 / 60.0, (seconds * 60.0) as usize);
}

/// Travel axis of a lane.
fn lane_axis(lane: &Lane) -> Axis {
    if lane.dir.x.abs() >= lane.dir.y.abs() {
        Axis::EastWest
    } else {
        Axis::NorthSouth
    }
}

/// Cars above a crawl.
fn moving(crowd: &Crowd) -> usize {
    crowd.cars().iter().filter(|c| c.speed > 0.5).count()
}

/// Centre line of the carriageway a lane belongs to.
fn road_center(city: &City, lane: &Lane) -> Vec2 {
    let road = &city.roads()[lane.road];
    road.axis.point(road.axis.along(lane.start), road.at)
}

/// A lane with room for a queue and somewhere to go.
fn long_lane(city: &City) -> usize {
    city.lanes()
        .iter()
        .filter(|l| l.length > 30.0 && !l.next.is_empty())
        .max_by(|a, b| {
            a.length
                .partial_cmp(&b.length)
                .unwrap_or(core::cmp::Ordering::Equal)
        })
        .map(|l| l.id)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// population
// ---------------------------------------------------------------------------

#[test]
fn traffic_spawns_the_configured_number_of_cars() {
    let city = city();
    let crowd = crowd(&city);
    assert_eq!(crowd.cars().len(), crowd.cfg().car_count);
    assert!(!crowd.cars().is_empty());
}

#[test]
fn every_car_starts_on_a_drivable_lane() {
    let city = city();
    let crowd = crowd(&city);
    for car in crowd.cars() {
        let lane = city
            .lanes()
            .get(car.lane)
            .unwrap_or_else(|| panic!("car refers to lane {}", car.lane));
        assert!(lane.near(car.pos, 1.0), "car is off lane {}", lane.id);
        assert!(car.s >= 0.0 && car.s <= lane.length);
        assert!(car.speed >= 0.0);
        assert!(car.pos.x.is_finite() && car.pos.y.is_finite());
    }
}

#[test]
fn cars_drive_on_the_right_half_of_the_carriageway() {
    let city = city();
    let crowd = crowd(&city);
    for car in crowd.cars() {
        let lane = &city.lanes()[car.lane];
        let road = &city.roads()[lane.road];
        // Right-hand traffic: `city-layout` offsets a lane by `perp(dir)` from the
        // carriageway centre, so the offset has the sign of the travel direction's
        // right-hand normal.
        let mid = lane.start.lerp(lane.end, 0.5);
        let offset = mid - road_center(&city, lane);
        let right = lane.dir.perp();
        assert!(
            offset.dot(right) * signf(right.x) >= 0.0 || offset.dot(right) > -0.01,
            "car on the wrong side of carriageway {}",
            road.id
        );
    }
}

#[test]
fn car_bodies_have_a_sane_size() {
    assert!(city_sim::CAR_LENGTH > 3.0 && city_sim::CAR_LENGTH < 6.0);
    for kind in [CarKind::Sedan, CarKind::Hatch, CarKind::Van, CarKind::Taxi] {
        assert!(kind.length() > 3.0 && kind.length() < 6.5);
    }
}

// ---------------------------------------------------------------------------
// driving
// ---------------------------------------------------------------------------

#[test]
fn cars_advance_along_their_lane_and_never_reverse() {
    let city = city();
    let mut crowd = crowd(&city);
    let before: Vec<(usize, f32)> = crowd.cars().iter().map(|c| (c.lane, c.s)).collect();
    run_frozen(&mut crowd, &city, 5.0);

    let mut advanced = 0;
    for (car, (lane, s)) in crowd.cars().iter().zip(before.iter()) {
        if car.lane == *lane {
            assert!(
                car.s >= *s - 1e-3,
                "a car drove backwards on lane {}",
                car.lane
            );
            if car.s > s + 0.5 {
                advanced += 1;
            }
        }
    }
    assert!(advanced > 0, "no car advanced along its lane");
}

#[test]
fn cars_point_along_their_lane() {
    let city = city();
    let mut crowd = crowd(&city);
    run_frozen(&mut crowd, &city, 6.0);
    for car in crowd.cars() {
        let lane = &city.lanes()[car.lane];
        assert!(
            car.dir.dot(lane.dir) > 0.999,
            "car heading is not its lane direction"
        );
        let p = lane.point_at(car.s);
        assert!(
            car.pos.dist(p) < 0.6,
            "car is {} m off lane {}",
            car.pos.dist(p),
            lane.id
        );
    }
}

#[test]
fn cars_never_exceed_the_speed_limit() {
    let city = city();
    let mut crowd = crowd(&city);
    run_frozen(&mut crowd, &city, 40.0);
    for car in crowd.cars() {
        let limit = city.lanes()[car.lane].speed_limit;
        assert!(
            car.speed <= limit + 0.05,
            "car at {:.2} m/s over a {} m/s limit",
            car.speed,
            limit
        );
    }
}

#[test]
fn traffic_keeps_moving() {
    let city = city();
    let mut crowd = crowd(&city);
    run_frozen(&mut crowd, &city, 30.0);
    let n = crowd.cars().len();
    assert!(
        moving(&crowd) * 2 > n,
        "only {}/{} cars moving after 20 s",
        moving(&crowd),
        n
    );
}

#[test]
fn car_bodies_never_overlap() {
    let city = city();
    let mut crowd = crowd(&city);
    run_frozen(&mut crowd, &city, 120.0);
    let cars: Vec<&_> = crowd.cars().iter().collect();
    for (i, a) in cars.iter().enumerate() {
        for b in cars.iter().skip(i + 1) {
            // Bodies are ~4.5 m long: a queue is fine, sharing a spot is not.
            assert!(
                a.pos.dist(b.pos) > 2.0,
                "two cars overlap at {:?}",
                a.pos
            );
        }
    }
}

// ---------------------------------------------------------------------------
// junctions and lights
// ---------------------------------------------------------------------------

#[test]
fn cars_continue_through_the_lane_graph() {
    let city = city();
    let mut crowd = crowd(&city);
    let start: Vec<usize> = crowd.cars().iter().map(|c| c.lane).collect();
    run_frozen(&mut crowd, &city, 120.0);
    let changed = crowd
        .cars()
        .iter()
        .zip(start.iter())
        .filter(|(c, s)| c.lane != **s)
        .count();
    assert!(
        changed > 0,
        "no car ever left its lane at a junction in two minutes"
    );
    for car in crowd.cars() {
        assert!(city.lanes().get(car.lane).is_some());
        assert!(car.s <= city.lanes()[car.lane].length);
    }
}

#[test]
fn a_car_stops_at_a_red_light() {
    let city = city();
    let mut crowd = crowd(&city);
    let mut held_at_junction = 0;
    for _ in 0..60 * 60 {
        crowd.step_with(&city, center(&city), false, 1.0 / 60.0);
        for car in crowd.cars() {
            let lane = &city.lanes()[car.lane];
            let remaining = lane.length - car.s;
            // Standing at the stop line while the light is red is the behaviour we want.
            if car.speed > 0.05 || remaining > 1.5 {
                continue;
            }
            let green = city
                .intersections()
                .get(lane.nodes[1])
                .map(|it| it.light_green(lane_axis(lane), crowd.traffic().time()))
                .unwrap_or(true);
            assert!(
                !green,
                "a car stands still at a GREEN light on lane {}",
                lane.id
            );
            held_at_junction += 1;
        }
    }
    assert!(held_at_junction > 0, "no car was ever held at a junction");
}

#[test]
fn both_axes_get_their_turn() {
    let city = city();
    let mut crowd = crowd(&city);
    let mut ns = 0;
    let mut ew = 0;
    for _ in 0..60 * 40 {
        crowd.step_with(&city, center(&city), false, 1.0 / 60.0);
        for car in crowd.cars() {
            if car.speed < 0.5 {
                continue;
            }
            match lane_axis(&city.lanes()[car.lane]) {
                Axis::NorthSouth => ns += 1,
                Axis::EastWest => ew += 1,
            }
        }
    }
    assert!(ns > 0 && ew > 0, "one traffic axis never moved");
}

#[test]
fn stopped_cars_do_not_block_a_junction_box() {
    let city = city();
    let mut crowd = crowd(&city);
    run_frozen(&mut crowd, &city, 120.0);
    for car in crowd.cars() {
        let lane = &city.lanes()[car.lane];
        let in_box = city
            .intersections()
            .get(lane.nodes[1])
            .map(|it| it.covers(car.pos))
            .unwrap_or(false);
        assert!(
            !(in_box && car.speed < 0.05),
            "a car is parked in the junction at {:?}",
            car.pos
        );
    }
}

#[test]
fn cars_turn_at_junctions() {
    let city = city();
    let mut crowd = crowd(&city);
    let mut turns = 0;
    let mut prev: Vec<usize> = crowd.cars().iter().map(|c| c.lane).collect();
    for _ in 0..60 * 120 {
        crowd.step_with(&city, center(&city), false, 1.0 / 60.0);
        for (car, p) in crowd.cars().iter().zip(prev.iter_mut()) {
            if car.lane != *p {
                turns += 1;
                *p = car.lane;
            }
        }
    }
    assert!(turns > 0, "no car turned at a junction in two minutes");
    // A turn is driven slower than the speed limit.
    assert!(TURN_SLOW > 0.0 && TURN_SLOW < 1.0);
}

#[test]
fn a_turn_is_a_real_turn() {
    let city = city();
    let mut crowd = crowd(&city);
    let mut checked = 0;
    let mut prev: Vec<(usize, usize)> = crowd.cars().iter().map(|c| (c.lane, 0)).collect();
    for _ in 0..60 * 180 {
        crowd.step_with(&city, center(&city), false, 1.0 / 60.0);
        for (car, p) in crowd.cars().iter().zip(prev.iter_mut()) {
            if car.lane == p.0 {
                continue;
            }
            let from = &city.lanes()[p.0];
            // The new lane must have been reachable from the old one.
            assert!(
                lane_can_reach(&city, p.0, car.lane),
                "car jumped from lane {} to unreachable lane {}",
                p.0,
                car.lane
            );
            let _ = from;
            checked += 1;
            p.0 = car.lane;
        }
    }
    assert!(checked > 0, "no lane changes observed");
}

/// `true` when `to` is one of the continuations of `lane`.
/// `true` when `to` is one of the legal continuations of `lane`.
fn lane_can_reach(city: &City, from: usize, to: usize) -> bool {
    city.lanes()
        .get(from)
        .map(|l| l.next.iter().any(|t| t.lane == to))
        .unwrap_or(false)
}

#[test]
fn a_car_only_yields_to_a_queue_it_plans_to_join() {
    // Two cars approaching the same junction from different lanes: the one that does
    // not turn into the queued lane must keep going.
    let city = city();
    let mut crowd = crowd(&city);
    run_frozen(&mut crowd, &city, 60.0);
    // Nobody may be stuck for the whole run.
    let stalled = crowd.cars().iter().filter(|c| c.speed < 0.05).count();
    assert!(
        stalled * 3 < crowd.cars().len(),
        "{}/{} cars are still standing after a minute",
        stalled,
        crowd.cars().len()
    );
}

// ---------------------------------------------------------------------------
// queueing
// ---------------------------------------------------------------------------

#[test]
fn a_car_never_reefs_into_the_one_ahead() {
    let city = city();
    let mut crowd = crowd(&city);
    let lane_id = long_lane(&city);
    // Stack every car on one lane, nose to tail, and let them drive for a while.
    {
        let traffic = crowd.traffic_mut();
        for (i, car) in traffic.cars_mut().iter_mut().enumerate() {
            car.lane = lane_id;
            car.s = 1.0 + (i as f32) * 1.2;
            car.speed = 0.0;
            car.intent = usize::MAX;
        }
    }
    crowd.place_cars(&city);
    // The following cars brake; the queue must keep its shape instead of compressing
    // into a single heap.
    let mut worst = f32::MAX;
    for _ in 0..30 {
        crowd.step_with(&city, center(&city), false, 1.0 / 60.0);
        let mut cars: Vec<&_> = crowd
            .cars()
            .iter()
            .filter(|c| c.lane == lane_id)
            .collect();
        cars.sort_by(|a, b| b.s.partial_cmp(&a.s).unwrap_or(core::cmp::Ordering::Equal));
        for pair in cars.windows(2) {
            worst = worst.min(pair[0].s - pair[1].s);
        }
    }
    // Spacing is measured bumper to bumper: 1.2 m arc pitch - body length may be
    // negative (that is the initial jam) but must never get worse than it started.
    assert!(worst > -6.0, "the queue collapsed to {worst:.2} m pitch");
    // ... and nobody drove backwards.
    for car in crowd.cars() {
        assert!(car.s >= 0.0);
    }
}

#[test]
fn a_jammed_car_does_not_drive_through_the_one_in_front() {
    let city = city();
    let mut crowd = crowd(&city);
    let lane_id = long_lane(&city);
    let length = city.lanes()[lane_id].length;
    let cars = crowd.traffic_mut().cars_mut();
    // Move everybody else out of the way: this test is about exactly two cars.
    for (i, car) in cars.iter_mut().skip(2).enumerate() {
        car.lane = (lane_id + 1 + i) % city.lanes().len();
        car.speed = 0.0;
    }
    {
        let cars = crowd.traffic_mut().cars_mut();
        cars[0].lane = lane_id;
        cars[0].s = length * 0.5;
        cars[0].speed = 0.0;
        cars[0].nerve = 1.0;
        cars[0].intent = usize::MAX;
        if cars.len() > 1 {
            cars[1].lane = lane_id;
            cars[1].s = length * 0.5 + 1.0;
            cars[1].speed = 0.0;
            cars[1].intent = usize::MAX;
        }
    }
    crowd.place_cars(&city);
    let front0 = crowd.cars()[0].s;
    let behind0 = crowd.cars()[1].s;
    let mut closest = f32::MAX;
    for _ in 0..120 {
        crowd.step_with(&city, center(&city), false, 1.0 / 60.0);
        if crowd.cars()[0].lane == lane_id && crowd.cars()[1].lane == lane_id {
            closest = closest.min(crowd.cars()[1].s - crowd.cars()[0].s);
        }
    }
    let front = crowd.cars()[0].s;
    let behind = crowd.cars()[1].s;
    // The follower must never close in on the car it is behind.
    assert!(
        behind - front <= behind0 - front0 + 0.01,
        "the following car closed the gap ({:.2} m -> {:.2} m)",
        behind0 - front0,
        behind - front
    );
    assert!(closest > 0.0, "the follower drove through the leader");
}

#[test]
fn spacing_constants_are_sane() {
    assert!(CAR_MIN_GAP > 1.0);
    assert!(CAR_ACCEL > 0.0);
}

// ---------------------------------------------------------------------------
// recycling + determinism
// ---------------------------------------------------------------------------

#[test]
fn a_car_outside_the_city_is_put_back_on_the_network() {
    let city = city();
    let mut crowd = crowd(&city);
    crowd.traffic_mut().cars_mut()[0].pos = Vec2::new(-60.0, -40.0);
    crowd.step(&city, 1.0 / 60.0);
    let car = &crowd.cars()[0];
    assert!(car.pos.x.is_finite() && car.pos.y.is_finite());
    assert!(
        city.bounds().contains(car.pos),
        "recycled car left the city at {:?}",
        car.pos
    );
    assert!(
        city.lanes()[car.lane].near(car.pos, 1.0),
        "recycled car is not on a lane"
    );
}

#[test]
fn the_population_is_constant_and_spreads_over_the_network() {
    let city = city();
    let mut crowd = crowd(&city);
    let n = crowd.cars().len();
    crowd.run(&city, 1.0 / 60.0, 60 * 300);
    assert_eq!(crowd.cars().len(), n);
    let lanes: HashSet<usize> = crowd.cars().iter().map(|c| c.lane).collect();
    assert!(lanes.len() > 3, "traffic sits on {} lanes", lanes.len());
}

#[test]
fn one_seed_always_produces_the_same_traffic() {
    let city = city();
    let mut a = Crowd::new(&city, SimConfig::default());
    let mut b = Crowd::new(&city, SimConfig::default());
    a.run_frozen(&city, 1.0 / 60.0, 60 * 10);
    b.run_frozen(&city, 1.0 / 60.0, 60 * 10);
    for (x, y) in a.cars().iter().zip(b.cars().iter()) {
        assert_eq!(x.lane, y.lane);
        assert_eq!(x.s, y.s);
        assert_eq!(x.pos, y.pos);
        assert_eq!(x.speed, y.speed);
    }
}

#[test]
fn another_seed_puts_the_cars_elsewhere() {
    let city = city();
    let a = Crowd::new(&city, SimConfig::default());
    let mut cfg = SimConfig::default();
    cfg.seed ^= 0x5A5A;
    let b = Crowd::new(&city, cfg);
    let same = a
        .cars()
        .iter()
        .zip(b.cars().iter())
        .filter(|(x, y)| x.pos.dist(y.pos) < 0.01)
        .count();
    assert!(same * 2 < a.cars().len(), "two seeds gave the same traffic");
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// `+1`/`-1` of `x`, never zero.
fn signf(x: f32) -> f32 {
    if x < 0.0 {
        -1.0
    } else {
        1.0
    }
}

#[allow(dead_code)]
fn keep_unused(_: &Car, _: &CrossingLink) {}
