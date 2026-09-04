//! Traffic: cars driving the lane graph, stopping at lights and at queues.
//!
//! A car is `(lane, s)` — an arc position on one [`city_layout::Lane`] — plus a little
//! longitudinal dynamics. Motion is 1-D along the lane; at the end of a lane the driver
//! picks a continuation out of `lane.next` (straight preferred, turns possible, U-turns
//! nearly never). Whether a car may enter the downstream junction is decided by the
//! light cycle owned by [`city_layout::Intersection`], and a queue propagates backwards
//! through [`gap_ahead`] — so an avenue fills up over a few blocks instead of teleporting.

use city_layout::{Axis, City, Lane};
use city_math::{move_towards, Rng, Vec2};

use crate::spawn;
use crate::SimConfig;

/// Stand-still stand-off behind a stopped car / at a light (m).
pub const CAR_MIN_GAP: f32 = 2.4;
/// Cornering speed factor when turning (relative to the speed limit).
pub const TURN_SLOW: f32 = 0.45;
/// Distance from a junction inside which a red light is obeyed (m).
pub const LIGHT_SIGHT: f32 = 18.0;
/// Distance from a junction inside which a turn is prepared for (m).
pub const TURN_SIGHT: f32 = 16.0;
/// Acceleration (m/s²) towards the desired speed.
pub const CAR_ACCEL: f32 = 3.2;
/// Braking deceleration (m/s²) used when the gap closes.
pub const CAR_BRAKE: f32 = 6.0;
/// Probability of actually turning when a turn is prepared.
pub const TURN_CHANCE: f32 = 0.6;
/// Nominal car length (m) for spacing.
pub const CAR_LENGTH: f32 = 4.4;
/// Half width of a car (m) — used by the renderer when it draws the body.
#[allow(dead_code)]
pub const CAR_HALF_WIDTH: f32 = 0.95;

/// Body style of a car (affects only its length and look).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarKind {
    Sedan,
    Hatch,
    Van,
    Taxi,
}

impl CarKind {
    /// Body length in metres.
    #[inline]
    pub fn length(self) -> f32 {
        match self {
            CarKind::Sedan => 4.6,
            CarKind::Hatch => 4.2,
            CarKind::Van => 5.2,
            CarKind::Taxi => 4.8,
        }
    }
}

/// One simulated car.
#[derive(Clone, Debug)]
pub struct Car {
    /// Lane currently driven.
    pub lane: usize,
    /// Arc position along that lane.
    pub s: f32,
    /// World XZ position (centre of the body).
    pub pos: Vec2,
    /// Heading (unit vector).
    pub dir: Vec2,
    /// Current speed (m/s).
    pub speed: f32,
    /// Driver's nerve: fraction of the speed limit the car aims for (`0.7..1`).
    pub nerve: f32,
    /// `true` while held before a red light or a queue.
    pub braking: bool,
    /// Lane id the driver intends to take at the next junction (`usize::MAX` = none).
    pub intent: usize,
    /// Frames of simulation time this car spent standing still (watchdog for gridlock).
    pub stuck: f32,
    /// Body style.
    pub kind: CarKind,
    /// Per-agent variation tag for the renderer (paint variety).
    pub variant: u8,
}

/// The traffic sub-model.
#[derive(Clone, Debug)]
pub struct Traffic {
    cars: Vec<Car>,
    /// Light-clock time, advanced with every [`Traffic::step`] so the junction lights
    /// follow simulation time rather than the wall clock.
    time: f32,
}

impl Traffic {
    /// Spawn `cfg.car_count` cars spread over the lane network.
    pub fn new(city: &City, cfg: &SimConfig, rng: &mut Rng) -> Traffic {
        let mut cars = Vec::new();
        spawn_all(city, cfg, rng, &mut cars);
        Traffic { cars, time: 0.0 }
    }

    /// Live cars.
    #[inline]
    pub fn cars(&self) -> &[Car] {
        &self.cars
    }
    /// Live cars (mutable — debug tools and tests place cars by hand).
    #[inline]
    pub fn cars_mut(&mut self) -> &mut Vec<Car> {
        &mut self.cars
    }
    /// Simulation time of the traffic clock (s).
    #[inline]
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Step every car by `dt`.
    pub fn step(&mut self, city: &City, rng: &mut Rng, dt: f32) {
        self.step_with(city, rng, true, dt);
    }

    /// Step every car by `dt`; `recycle == false` keeps cars out of the respawn ring.
    pub fn step_with(&mut self, city: &City, rng: &mut Rng, recycle: bool, dt: f32) {
        self.time += dt;
        for idx in 0..self.cars.len() {
            step_one(city, rng, &mut self.cars, idx, self.time, dt);
        }
        let _ = recycle;
    }

    /// Recycle the cars outside the live window around `focus`; returns the count.
    pub fn recycle(&mut self, city: &City, rng: &mut Rng, focus: Vec2) -> usize {
        let mut n = 0;
        for car in self.cars.iter_mut() {
            if spawn::should_recycle(city, focus, car.pos) {
                relocate(car, city, rng, focus);
                n += 1;
            }
        }
        n
    }
}

// ---------------------------------------------------------------------------
// spawning / recycling
// ---------------------------------------------------------------------------

/// Spawn `cfg.car_count` cars spread over the lane network.
pub fn spawn_all(city: &City, cfg: &SimConfig, rng: &mut Rng, out: &mut Vec<Car>) {
    let lanes: Vec<&Lane> = city.lanes().iter().filter(|l| lane_ok(l)).collect();
    if lanes.is_empty() {
        return;
    }
    for _ in 0..cfg.car_count {
        let lane = lanes[rng.index(lanes.len())];
        let mut car = make_car(lane.id, free_arc(lane, rng), rng);
        place(&mut car, city);
        out.push(car);
    }
}

/// A lane is driveable when it has some length and somewhere to go.
#[inline]
fn lane_ok(lane: &Lane) -> bool {
    lane.length > CAR_LENGTH * 2.0 && !lane.next.is_empty()
}

/// A random arc position that keeps the car on the lane.
fn free_arc(lane: &Lane, rng: &mut Rng) -> f32 {
    rng.range_f32(1.0, lane.length.max(2.0) - 1.0)
}

fn make_car(lane: usize, s: f32, rng: &mut Rng) -> Car {
    let kind = match rng.index(4) {
        0 => CarKind::Hatch,
        1 => CarKind::Van,
        2 => CarKind::Taxi,
        _ => CarKind::Sedan,
    };
    Car {
        lane,
        s,
        pos: Vec2::ZERO,
        dir: Vec2::X,
        speed: 0.0,
        nerve: rng.range_f32(0.72, 1.0),
        braking: false,
        intent: usize::MAX,
        stuck: 0.0,
        kind,
        variant: rng.index(6) as u8,
    }
}

/// Recompute world position/heading from the lane.
pub fn place(car: &mut Car, city: &City) {
    if let Some(lane) = city.lanes().get(car.lane) {
        car.pos = lane.point_at(car.s);
        car.dir = lane.dir;
    }
}

/// Re-place a car: prefer the respawn annulus around `focus`, else the lane closest to
/// `focus` (small city ⇒ the crowd simply stays where it is, on a new arc).
pub fn relocate(car: &mut Car, city: &City, rng: &mut Rng, focus: Vec2) {
    let ids = spawn::candidate_lanes(city, focus);
    let lane_id = if ids.is_empty() {
        nearest_lane(city, focus)
    } else {
        ids[rng.index(ids.len())]
    };
    let s = city
        .lanes()
        .get(lane_id)
        .map(|l| free_arc(l, rng))
        .unwrap_or(0.0);
    *car = make_car(lane_id, s, rng);
    place(car, city);
}

/// Driveable lane whose mid point is closest to `p`.
fn nearest_lane(city: &City, p: Vec2) -> usize {
    let mut best = 0;
    let mut bd = f32::MAX;
    for lane in city.lanes() {
        if !lane_ok(lane) {
            continue;
        }
        let d = lane.start.lerp(lane.end, 0.5).dist(p);
        if d < bd {
            bd = d;
            best = lane.id;
        }
    }
    best
}

// ---------------------------------------------------------------------------
// stepping
// ---------------------------------------------------------------------------

/// Standing this long without moving means the driver cannot get out of the jam (e.g. a
/// car parked in a junction box that the light cycle will not clear soon): the driver
/// gives up, turns around and drives away on the opposite lane of the same road.
const GRIDLOCK_SECONDS: f32 = 12.0;

/// Step one car: pick a speed, advance along the lane, maybe change lane.
fn step_one(city: &City, rng: &mut Rng, cars: &mut [Car], idx: usize, time: f32, dt: f32) {
    let mut car = cars[idx].clone();
    let Some(lane) = city.lanes().get(car.lane).cloned() else {
        return;
    };
    let target = desired_speed(city, cars, idx, &lane, time);
    car.speed = accelerate(car.speed, target, dt);
    car.braking = target < car.speed - 1e-3;
    car.stuck = if car.speed < 0.15 {
        car.stuck + dt
    } else {
        0.0
    };

    // Turning is decided once, close to the junction, and then driven.
    prepare_turn(city, rng, &mut car, &lane);

    car.s += car.speed * dt;
    if car.s >= lane.length {
        let next = take_turn(rng, &mut car, &lane);
        car.lane = next;
        car.s = 0.0;
    } else if car.stuck > GRIDLOCK_SECONDS {
        unstick(city, rng, &mut car, &lane);
    }
    if let Some(l) = city.lanes().get(car.lane) {
        car.pos = l.point_at(car.s);
        car.dir = l.dir;
    }
    cars[idx] = car;
}

/// Desired speed: limit × nerve, capped by a red light, by a turn, by the queue.
fn desired_speed(city: &City, cars: &[Car], idx: usize, lane: &Lane, time: f32) -> f32 {
    let car = &cars[idx];
    let mut v = lane.speed_limit * car.nerve;
    let remaining = lane.length - car.s;

    // Turn ahead: slow down while the junction is near.
    if car.intent != usize::MAX && remaining < TURN_SIGHT {
        v = v.min(lane.speed_limit * TURN_SLOW);
    }

    // Red light at the downstream junction (only near the stop line).
    if remaining <= LIGHT_SIGHT {
        let green = city
            .intersections()
            .get(lane.nodes[1])
            .map(|it| it.light_green(axis_of(lane), time))
            .unwrap_or(true);
        if !green {
            v = v.min(gap_speed(remaining, CAR_MIN_GAP));
        }
    }

    // Queue: brake for the nearest car ahead in the same lane.
    if let Some(gap) = gap_ahead(cars, idx, lane.id) {
        v = v.min(gap_speed(gap, CAR_MIN_GAP));
    }

    // ...and for a car queued in the *intended* lane just past the junction. Without
    // this the queue never propagates through a junction: every car would shoot into
    // the box and sit there until the light changes again. A car only yields to a queue
    // it actually intends to join.
    if car.intent != usize::MAX {
        if let Some(front) = first_car_in(cars, idx, car.intent) {
            // stop distance = what is left of this lane + the stand-off behind `front`
            v = v.min(gap_speed(remaining + front, CAR_MIN_GAP));
        }
    }
    v.max(0.0)
}

/// Arc position (`s`) of the first car driving `lane` — the tail of its queue.
fn first_car_in(cars: &[Car], idx: usize, lane: usize) -> Option<f32> {
    cars.iter()
        .enumerate()
        .filter(|(j, c)| *j != idx && c.lane == lane)
        .map(|(_, c)| c.s)
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
}

/// Travel axis of a lane.
#[inline]
fn axis_of(lane: &Lane) -> Axis {
    if lane.dir.x.abs() >= lane.dir.y.abs() {
        Axis::EastWest
    } else {
        Axis::NorthSouth
    }
}

/// Speed that still stops within `d` metres of the stand-off point.
fn gap_speed(distance: f32, stand_off: f32) -> f32 {
    let d = (distance - stand_off).max(0.0);
    (2.0 * CAR_BRAKE * d).sqrt()
}

/// Gap (m) to the closest car ahead of `idx` in the same lane.
fn gap_ahead(cars: &[Car], idx: usize, lane: usize) -> Option<f32> {
    let me = &cars[idx];
    let mut best: Option<f32> = None;
    for (j, other) in cars.iter().enumerate() {
        if j == idx || other.lane != lane {
            continue;
        }
        let d = other.s - me.s - other.kind.length();
        if d > 0.0 && best.map(|bd| d < bd).unwrap_or(true) {
            best = Some(d);
        }
    }
    best
}

/// Ease the speed towards `target`: gentle throttle, firmer brakes.
fn accelerate(v: f32, target: f32, dt: f32) -> f32 {
    if target > v {
        move_towards(v, target, CAR_ACCEL * dt)
    } else {
        move_towards(v, target, CAR_BRAKE * dt)
    }
}

/// Decide (once) which lane to take at the upcoming junction.
fn prepare_turn(city: &City, rng: &mut Rng, car: &mut Car, lane: &Lane) {
    let remaining = lane.length - car.s;
    if car.intent != usize::MAX || remaining > TURN_SIGHT || lane.next.is_empty() {
        return;
    }
    // Only commit to a turn when a turn is actually available.
    let has_turn = lane.next.iter().any(|t| is_turn(lane, t.lane, city));
    if has_turn && rng.chance(TURN_CHANCE) {
        let turns: Vec<usize> = lane
            .next
            .iter()
            .filter(|t| is_turn(lane, t.lane, city))
            .map(|t| t.lane)
            .collect();
        car.intent = turns[rng.index(turns.len())];
    } else {
        // Straight on (or the only option).
        car.intent = lane
            .next
            .iter()
            .max_by(|a, b| {
                a.weight
                    .partial_cmp(&b.weight)
                    .unwrap_or(core::cmp::Ordering::Equal)
            })
            .map(|t| t.lane)
            .unwrap_or(usize::MAX);
    }
}

/// `true` when continuing from `lane` to `target` means turning (not straight on).
fn is_turn(lane: &Lane, target: usize, city: &City) -> bool {
    let Some(dir) = city.lanes().get(target).map(|l| l.dir) else {
        return false;
    };
    lane.dir.dot(dir) < 0.98
}

/// Commit to the intended lane at the end of the current one.
fn take_turn(rng: &mut Rng, car: &mut Car, lane: &Lane) -> usize {
    let intent = car.intent;
    car.intent = usize::MAX;
    if intent != usize::MAX && lane.next.iter().any(|t| t.lane == intent) {
        return intent;
    }
    // No valid intent (dead end, or the intent was not reachable): pick by weight.
    pick_weighted(lane, rng)
}

/// Escape gridlock: reverse onto the opposite carriageway of the same road.
fn unstick(city: &City, rng: &mut Rng, car: &mut Car, lane: &Lane) {
    // The other direction of the same carriageway.
    let other = city.lanes().get(road_opposite(lane)).cloned();
    if let Some(back) = other {
        car.lane = back.id;
        // Start at the far end of the opposite lane and drive away from the jam.
        car.s = back.length * rng.range_f32(0.35, 0.9);
        car.speed = 0.0;
        car.stuck = 0.0;
        car.intent = usize::MAX;
        car.pos = back.point_at(car.s);
        car.dir = back.dir;
    }
    let _ = lane;
}

/// Id of the lane driving the other way on the same carriageway.
#[inline]
fn road_opposite(lane: &Lane) -> usize {
    // `city-layout` pushes the two directions of a carriageway as a pair, so the lane
    // driving the other way on the same road is the sibling id.
    lane.id ^ 1
}

/// Weighted random continuation of a lane (straight is favoured by `city-layout`).
fn pick_weighted(lane: &Lane, rng: &mut Rng) -> usize {
    if lane.next.is_empty() {
        return lane.id;
    }
    let total: f32 = lane.next.iter().map(|t| t.weight.max(0.0)).sum();
    let mut pick = rng.range_f32(0.0, total.max(1e-3));
    for t in &lane.next {
        if t.weight <= 0.0 {
            continue;
        }
        pick -= t.weight;
        if pick <= 0.0 {
            return t.lane;
        }
    }
    lane.next
        .iter()
        .max_by(|a, b| {
            a.weight
                .partial_cmp(&b.weight)
                .unwrap_or(core::cmp::Ordering::Equal)
        })
        .map(|t| t.lane)
        .unwrap_or(lane.id)
}
