//! # city-sim
//!
//! Bounded context: *street life*. Nothing in the generated city moves; this crate
//! decides where every pedestrian and car is at every simulation instant, walking the
//! network that [city_layout](city_layout) generated: pedestrians follow the
//! [`city_layout::SidewalkLoop`]s and cross only at zebra crossings, cars follow
//! [`city_layout::Lane`]s and pick their next lane at each junction.
//!
//! Design notes:
//! * **The sim is a pure function of `(seed, city, dt)`** — no DOM, no wall clock — so
//!   it runs natively under `cargo test` and identically in the browser. The same seed
//!   and the same number of steps always produce the same crowd.
//! * **Parked flow:** each agent owns a lane (or sidewalk loop) plus an arc position;
//!   motion is 1-D along the lane and the graph `lane.next` decides where it continues.
//!   Steering stays O(1) per agent; there is no path search per frame.
//! * **Spacing** is enforced by forward collision: an agent brakes for the nearest
//!   neighbour ahead in its own lane and for cross traffic that is inside an
//!   intersection; cars stop at red lights and behind queued neighbours.
//! * **Off-screen management:** the crowd lives around the player; agents the player
//!   will never see are culled and respawned on the ring around the live window, far
//!   enough away that the pop-in is not visible.
//! * All spawn/respawn randomness comes from [`city_math::Rng`] seeded from
//!   [`SimConfig::seed`] — no platform RNG.
//!
//! ```
//! use city_layout::{City, CityParams};
//! use city_sim::{Crowd, SimConfig};
//!
//! let city = City::generate(CityParams::default());
//! let mut crowd = Crowd::new(&city, SimConfig::default());
//! for _ in 0..60 {
//!     crowd.step(&city, 1.0 / 60.0);
//! }
//! assert_eq!(crowd.peds().len(), crowd.cfg().ped_count);
//! assert!(crowd.cars().iter().all(|c| c.speed.is_finite()));
//! ```

#![forbid(unsafe_code)]

mod cars;
mod crowd;
mod spawn;

pub use cars::{Car, CarKind, Traffic, CAR_ACCEL, CAR_BRAKE, CAR_LENGTH, CAR_MIN_GAP, TURN_CHANCE, TURN_SIGHT, TURN_SLOW, LIGHT_SIGHT};
pub use crowd::{
    recycle_peds, relocate as relocate_ped, spawn_all as spawn_peds, step_peds, Ped, PedState,
    CONGEST_GAP, CROSS_CHANCE, CROSS_ENTRY, LANE_OFFSETS, MIN_GAP, PED_LANES, PED_RADIUS,
    PED_SPEED, PED_STRIDE, REST_CHANCE, REST_SECONDS,
};
pub use spawn::{
    DESPAWN_MARGIN, LIVE_RADIUS, RESPAWN_RING_NEAR, RESPAWN_RING_MAX, RING_SPAN,
};

use city_layout::City;
use city_math::{clamp, Rng, Vec2};

/// Tuning of the street simulation.
#[derive(Clone, Debug)]
pub struct SimConfig {
    /// Seed of all spawn/behaviour randomness.
    pub seed: u64,
    /// Number of pedestrians kept alive.
    pub ped_count: usize,
    /// Number of cars kept alive.
    pub car_count: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            seed: 0x51_17,
            ped_count: 48,
            car_count: 26,
        }
    }
}

impl SimConfig {
    /// A small configuration for tests.
    pub fn tiny() -> SimConfig {
        SimConfig {
            seed: 7,
            ped_count: 8,
            car_count: 5,
        }
    }
}

/// The whole street simulation: the pedestrian crowd plus [`Traffic`].
pub struct Crowd {
    cfg: SimConfig,
    peds: Vec<Ped>,
    traffic: Traffic,
    rng: Rng,
    time: f32,
    /// Point the live window is centred on (updated by [`Crowd::step_around`]).
    focus: Vec2,
    /// Agents recycled during the last step (spawns + despawns).
    recycled: usize,
}

impl Crowd {
    /// Spawn `ped_count` pedestrians and `car_count` cars on the network of `city`.
    pub fn new(city: &City, cfg: SimConfig) -> Crowd {
        let mut rng = Rng::new(cfg.seed);
        let traffic = cars::Traffic::new(city, &cfg, &mut rng);
        let mut peds = Vec::new();
        crowd::spawn_all(city, &cfg, &mut rng, &mut peds);
        let focus = spawn::city_focus(city);
        Crowd {
            cfg,
            peds,
            traffic,
            rng,
            time: 0.0,
            focus,
            recycled: 0,
        }
    }

    /// One simulation step of `dt` real seconds, centred on the city spawn point.
    pub fn step(&mut self, city: &City, dt: f32) {
        let focus = spawn::city_focus(city);
        self.step_around(city, focus, dt);
    }

    /// One step of `dt` with the live window centred on `focus` (the player position).
    pub fn step_around(&mut self, city: &City, focus: Vec2, dt: f32) {
        self.step_with(city, focus, true, dt);
    }

    /// One step of `dt` with the live window centred on `focus`.
    #[inline]
    pub fn step_at(&mut self, city: &City, focus: Vec2, dt: f32) {
        self.step_with(city, focus, true, dt);
    }

    /// One step of `dt`; `recycle == false` keeps every agent where it is. Tests and
    /// replays use this: nothing teleports, so positions stay comparable frame to frame.
    pub fn step_with(&mut self, city: &City, focus: Vec2, recycle: bool, dt: f32) {
        let dt = sim_dt(dt);
        self.time += dt;
        self.focus = focus;
        crowd::step_peds(city, &mut self.rng, &mut self.peds, self.traffic.cars(), dt);
        self.traffic.step_with(city, &mut self.rng, recycle, dt);
        self.recycled = if recycle {
            crowd::recycle_peds(city, &mut self.peds, &mut self.rng, focus)
                + self.traffic.recycle(city, &mut self.rng, focus)
        } else {
            0
        };
    }

    /// Run `steps` steps of `dt` (headless tests / replays).
    pub fn run(&mut self, city: &City, dt: f32, steps: usize) {
        for _ in 0..steps {
            self.step(city, dt);
        }
    }

    /// Run `steps` steps of `dt` with the live window kept on `focus`.
    pub fn run_around(&mut self, city: &City, focus: Vec2, dt: f32, steps: usize) {
        for _ in 0..steps {
            self.step_with(city, focus, true, dt);
        }
    }

    /// Run `steps` steps of `dt` without any recycling.
    pub fn run_frozen(&mut self, city: &City, dt: f32, steps: usize) {
        let focus = self.focus;
        for _ in 0..steps {
            self.step_with(city, focus, false, dt);
        }
    }

    /// The focus the live window is currently centred on.
    #[inline]
    pub fn focus(&self) -> Vec2 {
        self.focus
    }

    // --- accessors ------------------------------------------------------

    /// All live pedestrians.
    #[inline]
    pub fn peds(&self) -> &[Ped] {
        &self.peds
    }
    /// The traffic sub-model.
    #[inline]
    pub fn traffic(&self) -> &Traffic {
        &self.traffic
    }
    /// Live cars (world XZ position, heading, speed).
    #[inline]
    pub fn cars(&self) -> &[Car] {
        self.traffic.cars()
    }
    /// Configuration in use.
    #[inline]
    pub fn cfg(&self) -> &SimConfig {
        &self.cfg
    }
    /// Simulation time accumulated since construction (s).
    #[inline]
    pub fn time(&self) -> f32 {
        self.time
    }
    /// Agents respawned during the last step.
    #[inline]
    pub fn recycled_last_step(&self) -> usize {
        self.recycled
    }
    /// Live pedestrians (mutable — used by tests and by debug tools that want to place
    /// a walker by hand).
    #[inline]
    pub fn peds_mut(&mut self) -> &mut Vec<Ped> {
        &mut self.peds
    }
    /// Live traffic (mutable).
    #[inline]
    pub fn traffic_mut(&mut self) -> &mut Traffic {
        &mut self.traffic
    }
    /// Recompute a pedestrian's world position from its loop / arc / stance.
    pub fn place_ped(&self, city: &City, ped: &mut Ped) {
        crowd::place(city, ped);
    }

    /// Recompute the world position of every pedestrian from its loop / arc / stance.
    pub fn place_peds(&mut self, city: &City) {
        for ped in self.peds.iter_mut() {
            crowd::place(city, ped);
        }
    }

    /// Recompute one pedestrian by index (no-op when the index is out of range).
    pub fn place_ped_at(&mut self, city: &City, index: usize) {
        if let Some(ped) = self.peds.get_mut(index) {
            crowd::place(city, ped);
        }
    }

    /// Recompute the world position of every car from its lane / arc.
    pub fn place_cars(&mut self, city: &City) {
        for car in self.traffic.cars_mut() {
            cars::place(car, city);
        }
    }

    /// Recompute one car by index (no-op when the index is out of range).
    pub fn place_car_at(&mut self, city: &City, index: usize) {
        if let Some(car) = self.traffic.cars_mut().get_mut(index) {
            cars::place(car, city);
        }
    }

    /// Nearest pedestrian to `p` as `(index, distance)`, `None` when the crowd is empty.
    pub fn nearest_ped(&self, p: Vec2) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, ped) in self.peds.iter().enumerate() {
            let d = p.dist(ped.pos());
            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((i, d));
            }
        }
        best
    }

    /// Nearest car to `p` as `(index, distance)`, `None` when there is no traffic.
    pub fn nearest_car(&self, p: Vec2) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, car) in self.traffic.cars().iter().enumerate() {
            let d = p.dist(car.pos);
            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((i, d));
            }
        }
        best
    }
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

/// Clamp `dt` to a sane simulation step (mirrors the fixed step of `city-app`).
#[inline]
pub fn sim_dt(dt: f32) -> f32 {
    if dt.is_finite() {
        clamp(dt, 0.0, 0.1)
    } else {
        0.0
    }
}
