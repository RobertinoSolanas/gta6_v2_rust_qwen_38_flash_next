//! Pedestrians: walk a sidewalk loop, cross the street at a zebra crossing, repeat.
//!
//! A pedestrian is `(loop, s, offset)` — an arc position on a [`SidewalkLoop`] plus a
//! sideways stance offset — so "walking the pavement" is 1-D motion along a known curve
//! and needs no path search. Near the entry point of a [`city_layout::CrossingLink`] the
//! walker may hop to the neighbouring block's loop across a marked crossing; that is the
//! only routing decision, and it is deterministic given the RNG stream.
//!
//! Collision avoidance is two cheap rules instead of steering forces: slow down when the
//! neighbour *ahead in the same loop* closes in, and do not step onto a crossing while a
//! car is inside the stretch of road it crosses. Crowds therefore bunch up exactly where
//! a real crowd does — at the kerb of a busy avenue.

use city_layout::{City, CrossingLink, SidewalkLoop};
use city_math::{clamp, wrap_period, Rng, Vec2, TAU};

use crate::cars::Car;
use crate::spawn;
use crate::SimConfig;

/// Spacing radius of a pedestrian (m).
pub const PED_RADIUS: f32 = 0.35;
/// Number of side-by-side walking lines inside one sidewalk band.
pub const PED_LANES: usize = 3;
/// Stride length of the walk cycle (m): phase advances `speed / stride` cycles per s.
pub const PED_STRIDE: f32 = 1.7;
/// Nominal walking speed (m/s) before per-agent variation.
pub const PED_SPEED: f32 = 1.35;
/// The three walking lines inside a sidewalk band (m offset from its centre line).
pub const LANE_OFFSETS: [f32; 3] = [-0.6, 0.0, 0.6];
/// Distance ahead (m) at which a closing neighbour starts to slow a walker down.
pub const CONGEST_GAP: f32 = 2.2;
/// Shortest comfortable stand-off (m); at this gap the walker nearly stops.
pub const MIN_GAP: f32 = 0.7;
/// How close to the crossing entry (arc length, m) the hop decision is taken.
pub const CROSS_ENTRY: f32 = 1.6;
/// Probability of taking a crossing opportunity.
pub const CROSS_CHANCE: f32 = 0.5;
/// Chance to rest briefly at the far corner after a crossing.
pub const REST_CHANCE: f32 = 0.1;
/// Length of that rest (s).
pub const REST_SECONDS: f32 = 2.5;

/// What a pedestrian is doing right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PedState {
    /// Walking along a sidewalk loop.
    Walking,
    /// Midway across a carriageway on a marked crossing.
    Crossing,
    /// Standing still for a moment (corner wait, rest).
    Waiting,
}

/// One simulated pedestrian.
#[derive(Clone, Debug)]
pub struct Ped {
    /// Loop currently walked. While `Crossing`, the loop the current link starts on.
    pub loop_id: usize,
    /// Arc position on [`City::loops`]`[loop_id]`.
    pub s: f32,
    /// Sideways stance offset from the loop centre line (m; + = walker's right).
    pub offset: f32,
    /// World X (the city lives in the XZ ground plane).
    pub x: f32,
    /// World Z.
    pub z: f32,
    /// Heading (unit vector, XZ).
    pub dir: Vec2,
    /// Preferred walking speed (m/s).
    pub speed_pref: f32,
    /// Instantaneous speed (m/s) after congestion.
    pub speed: f32,
    /// What the pedestrian is doing.
    pub state: PedState,
    /// Link being walked when `state == Crossing`, else `usize::MAX` ("no crossing in
    /// sight"): the mark that makes every approach exactly one decision.
    pub link: usize,
    /// Progress `0..=1` along the current crossing.
    pub link_t: f32,
    /// `s` at which the last crossing was considered (diagnostics).
    pub link_from_s: f32,
    /// Walk-cycle phase in radians (drives the future humanoid rig).
    pub phase: f32,
    /// Seconds left standing still.
    pub idle: f32,
    /// Per-agent variation tag (0..8) for the future renderer.
    pub variant: u8,
}

impl Ped {
    /// World position (XZ).
    #[inline]
    pub fn pos(&self) -> Vec2 {
        Vec2::new(self.x, self.z)
    }
    /// `true` while crossing a carriageway.
    #[inline]
    pub fn crossing(&self) -> bool {
        self.state == PedState::Crossing
    }
    /// `true` while standing still.
    #[inline]
    pub fn idle_now(&self) -> bool {
        self.state == PedState::Waiting
    }
    /// Direction of travel (unit; stale while standing).
    #[inline]
    pub fn heading(&self) -> Vec2 {
        self.dir
    }
}

// ---------------------------------------------------------------------------
// spawning / recycling
// ---------------------------------------------------------------------------

/// Spawn `cfg.ped_count` pedestrians spread over the sidewalk network.
pub fn spawn_all(city: &City, cfg: &SimConfig, rng: &mut Rng, out: &mut Vec<Ped>) {
    let loops: Vec<&SidewalkLoop> = city
        .loops()
        .iter()
        .filter(|l| l.perimeter() > 10.0)
        .collect();
    if loops.is_empty() {
        return;
    }
    for _ in 0..cfg.ped_count {
        let loop_ = loops[rng.index(loops.len())];
        let mut ped = make_ped(loop_.id, rng.range_f32(0.0, loop_.perimeter()), rng);
        place(city, &mut ped);
        out.push(ped);
    }
}

/// A fresh pedestrian on `loop_id` at arc `s`, with a new random personality.
fn make_ped(loop_id: usize, s: f32, rng: &mut Rng) -> Ped {
    Ped {
        loop_id,
        s,
        offset: LANE_OFFSETS[rng.index(PED_LANES)],
        x: 0.0,
        z: 0.0,
        dir: Vec2::X,
        speed_pref: rng.range_f32(PED_SPEED * 0.8, PED_SPEED * 1.25),
        speed: 0.0,
        state: PedState::Walking,
        link: usize::MAX,
        link_t: 0.0,
        link_from_s: 0.0,
        phase: rng.range_f32(0.0, TAU),
        idle: 0.0,
        variant: rng.index(8) as u8,
    }
}

/// Recompute `x/z/dir` from the loop position and stance offset (no obstacle pass).
pub fn place(city: &City, ped: &mut Ped) {
    if let Some(loop_) = city.loops().get(ped.loop_id) {
        update_from_loop(ped, loop_);
    }
}

/// Refresh world position/heading of a walker from its loop, then keep it out of solid
/// geometry (see [`push_off_solids`]).
fn update_from_loop(ped: &mut Ped, loop_: &SidewalkLoop) {
    let p = loop_.point_at(ped.s);
    let d = loop_.dir_at(ped.s);
    ped.dir = d;
    let q = p + d.perp() * ped.offset;
    ped.x = q.x;
    ped.z = q.y;
}

/// Nudge a pedestrian out of solid geometry and back inside the city.
///
/// A sidewalk loop runs along the middle of the sidewalk band and a walker keeps to one
/// of three lines on it, so in a well generated city this never fires. It exists because
/// `city-layout` also scatters benches, bins and planters on the very same band: without
/// this, half the crowd would walk through the street furniture.
pub fn avoid_obstacles(city: &City, ped: &mut Ped) {
    if ped.state == PedState::Crossing {
        return; // mid-crossing the walker belongs on the tarmac
    }
    let fixed = city.resolve(ped.pos(), PED_RADIUS);
    ped.x = fixed.x;
    ped.z = fixed.y;
}

/// Re-place a pedestrian. When the respawn annulus around `focus` holds candidate
/// loops, pick one at a random arc; otherwise (small city, player downtown) jump to the
/// far side of the loop nearest the player so nothing pops in next to the camera.
pub fn relocate(ped: &mut Ped, city: &City, rng: &mut Rng, focus: Vec2) {
    let ids = spawn::candidate_loops(city, focus);
    let (loop_id, s) = if ids.is_empty() {
        let loop_ = nearest_loop(city, focus);
        (loop_.id, far_arc(loop_, focus))
    } else {
        let loop_ = &city.loops()[ids[rng.index(ids.len())]];
        (loop_.id, rng.range_f32(0.0, loop_.perimeter()))
    };
    *ped = make_ped(loop_id, s, rng);
    place(city, ped);
}

/// Recycle every pedestrian outside the live window; returns how many were replaced.
pub fn recycle_peds(city: &City, peds: &mut [Ped], rng: &mut Rng, focus: Vec2) -> usize {
    let mut n = 0;
    for ped in peds.iter_mut() {
        if spawn::should_recycle(city, focus, ped.pos()) {
            relocate(ped, city, rng, focus);
            n += 1;
        }
    }
    n
}

/// The loop whose polyline passes closest to `p`.
fn nearest_loop(city: &City, p: Vec2) -> &SidewalkLoop {
    let mut best = &city.loops()[0];
    let mut bd = f32::MAX;
    for loop_ in city.loops() {
        let d = loop_.project(p).2;
        if d < bd {
            bd = d;
            best = loop_;
        }
    }
    best
}

/// Arc on `loop_` whose point is farthest from `focus`.
fn far_arc(loop_: &SidewalkLoop, focus: Vec2) -> f32 {
    let mut best_s = 0.0;
    let mut best_d = -1.0;
    for (i, p) in loop_.points().iter().enumerate() {
        let d = focus.dist(*p);
        if d > best_d {
            best_d = d;
            best_s = loop_.cum[i.min(loop_.cum.len() - 1)];
        }
    }
    loop_.wrap(best_s + 1.0)
}

// ---------------------------------------------------------------------------
// stepping
// ---------------------------------------------------------------------------

/// Step every pedestrian by `dt` seconds. `cars` keeps walkers from stepping out in
/// front of traffic.
pub fn step_peds(city: &City, rng: &mut Rng, peds: &mut [Ped], cars: &[Car], dt: f32) {
    for idx in 0..peds.len() {
        step_one(city, rng, peds, idx, cars, dt);
    }
}

fn step_one(city: &City, rng: &mut Rng, peds: &mut [Ped], idx: usize, cars: &[Car], dt: f32) {
    let mut ped = peds[idx].clone();
    let v = match ped.state {
        PedState::Waiting => {
            ped.idle -= dt;
            if ped.idle <= 0.0 {
                ped.state = PedState::Walking;
            }
            0.0
        }
        PedState::Crossing => walk_crossing(city, rng, &mut ped, cars, dt),
        PedState::Walking => walk_loop(city, rng, peds, idx, &mut ped, cars, dt),
    };
    avoid_obstacles(city, &mut ped);
    ped.speed = v;
    advance_phase(&mut ped, v, dt);
    peds[idx] = ped;
}

/// Walk the current sidewalk loop; returns the speed actually walked.
fn walk_loop(
    city: &City,
    rng: &mut Rng,
    peds: &[Ped],
    idx: usize,
    ped: &mut Ped,
    cars: &[Car],
    dt: f32,
) -> f32 {
    let loop_ = &city.loops()[ped.loop_id.min(city.loops().len() - 1)];
    let mut v = ped.speed_pref;
    if let Some(gap) = gap_ahead(peds, idx, loop_.perimeter()) {
        let room = (gap - MIN_GAP) / (CONGEST_GAP - MIN_GAP);
        v *= clamp(room, 0.12, 1.0);
    }
    ped.s = loop_.wrap(ped.s + v * dt);
    update_from_loop(ped, loop_);
    consider_crossing(city, rng, ped, cars);
    v
}

/// Walk the current crossing; returns the speed walked (`0` = still held at the kerb).
fn walk_crossing(city: &City, rng: &mut Rng, ped: &mut Ped, cars: &[Car], dt: f32) -> f32 {
    let Some(link) = city.links().get(ped.link).cloned() else {
        ped.state = PedState::Walking;
        ped.link = usize::MAX;
        return 0.0;
    };
    let (near, far) = crossing_ends(city, &link);
    let len = (far - near).len().max(0.5);
    if ped.link_t <= 1e-4 && traffic_on_crossing(&link, cars) {
        // At the kerb: wait for a gap in the traffic before stepping off.
        return 0.0;
    }
    ped.link_t += ped.speed_pref.max(0.6) * dt / len;
    if ped.link_t >= 1.0 {
        ped.loop_id = link.to_loop;
        ped.s = link.to_s;
        ped.link = usize::MAX;
        ped.link_t = 0.0;
        ped.state = if rng.chance(REST_CHANCE) {
            ped.idle = REST_SECONDS;
            PedState::Waiting
        } else {
            PedState::Walking
        };
        if let Some(loop_) = city.loops().get(ped.loop_id) {
            update_from_loop(ped, loop_);
        }
        return ped.speed_pref;
    }
    let q = near.lerp(far, ped.link_t);
    ped.x = q.x;
    ped.z = q.y;
    ped.dir = (far - near).norm();
    ped.speed_pref
}

/// End points (world XZ) of a crossing: kerb to kerb.
fn crossing_ends(city: &City, link: &CrossingLink) -> (Vec2, Vec2) {
    let a = city.loops()[link.from_loop].point_at(link.from_s);
    let b = city.loops()[link.to_loop].point_at(link.to_s);
    (a, b)
}

/// `true` when a car is close enough to the crossing to make stepping out unsafe.
///
/// A car blocks the crossing when it drives *towards* it (so a car that already passed
/// does not freeze the walker) and is within braking reach of the crossing mid point.
fn traffic_on_crossing(link: &CrossingLink, cars: &[Car]) -> bool {
    cars.iter().any(|c| {
        let rel = link.mid - c.pos;
        let t = rel.dot(c.dir);
        t > 0.0 && t < 12.0 && (rel - c.dir * t).len() < 2.6
    })
}

/// Maybe take the crossing the walker is standing at.
///
/// The decision is taken once per approach: the link id is stored on the pedestrian and
/// ignored until the walker leaves the crossing area (or finishes crossing it).
fn consider_crossing(city: &City, rng: &mut Rng, ped: &mut Ped, cars: &[Car]) {
    let loop_ = &city.loops()[ped.loop_id.min(city.loops().len() - 1)];
    let mut hit: Option<usize> = None;
    let mut best = f32::MAX;
    for link in city.links() {
        if link.from_loop != ped.loop_id {
            continue;
        }
        let d = forward_delta(ped.s, link.from_s, loop_.perimeter());
        if d <= CROSS_ENTRY && d < best {
            best = d;
            hit = Some(link.id);
        }
    }
    let Some(id) = hit else {
        // Not standing at any crossing entry: forget the previous one.
        ped.link = usize::MAX;
        return;
    };
    if ped.link != usize::MAX {
        return; // this opportunity was already considered
    }
    ped.link = id;
    ped.link_from_s = ped.s;
    if !rng.chance(CROSS_CHANCE) {
        return;
    }
    if let Some(link) = city.links().get(id) {
        if traffic_on_crossing(link, cars) {
            return;
        }
        ped.state = PedState::Crossing;
        // `link_t == 0` means "at the kerb": put the walker where the crossing starts
        // instead of wherever on the approach arc it happened to be standing.
        let q = city
            .loops()
            .get(link.from_loop)
            .map(|l| l.point_at(link.from_s));
        if let Some(q) = q {
            ped.x = q.x;
            ped.z = q.y;
        }
        ped.link_t = 0.0;
    }
}

/// Signed arc distance still to walk from `from` to `target` on a closed loop.
#[inline]
fn forward_delta(from: f32, target: f32, perimeter: f32) -> f32 {
    wrap_period(target - from, perimeter)
}

/// Gap (m) to the nearest pedestrian *ahead* of `idx` on the same loop.
fn gap_ahead(peds: &[Ped], idx: usize, perimeter: f32) -> Option<f32> {
    let me = &peds[idx];
    if me.state != PedState::Walking {
        return None;
    }
    let mut best: Option<f32> = None;
    for (j, other) in peds.iter().enumerate() {
        if j == idx || other.loop_id != me.loop_id || other.state == PedState::Waiting {
            continue;
        }
        let d = forward_delta(me.s, other.s, perimeter);
        if d > 0.05 && best.map(|bd| d < bd).unwrap_or(true) {
            best = Some(d);
        }
    }
    best.filter(|d| *d <= CONGEST_GAP)
}

/// Advance the walk-cycle phase like `city-avatar`: cycles per second = speed / stride.
fn advance_phase(ped: &mut Ped, v: f32, dt: f32) {
    ped.phase = wrap_period(ped.phase + v * dt / PED_STRIDE * TAU, TAU);
    let _ = PED_RADIUS;
}
