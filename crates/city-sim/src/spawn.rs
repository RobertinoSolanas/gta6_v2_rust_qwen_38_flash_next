//! Live-agent lifecycle: where things spawn and when they are recycled.
//!
//! The crowd is not a simulation of the whole city — it is a stage crew that lives
//! around the player. Agents further from the focus than [`LIVE_RADIUS`] are teleported
//! onto the respawn annulus, which starts *beyond* the distance the eye (and the
//! camera) can reach, so the pop-in is never in view. When the player stands in the
//! middle of a small city every agent is "live" and nothing gets recycled — the ring
//! simply has no candidates and the crowd stays put.

use city_layout::{City, SidewalkLoop};
use city_math::Vec2;

/// Extra metres beyond the city border at which an agent is beyond saving.
pub const DESPAWN_MARGIN: f32 = 6.0;
/// Live window: agents closer to the focus than this always stay.
pub const LIVE_RADIUS: f32 = 160.0;
/// Respawn annulus, inner edge (never closer than this to the focus).
pub const RESPAWN_RING_NEAR: f32 = 150.0;
/// Outer edge of the respawn annulus.
pub const RESPAWN_RING_MAX: f32 = RESPAWN_RING_NEAR + RING_SPAN;
/// Width of the band searched for spawn sites beyond `RESPAWN_RING_NEAR`.
pub const RING_SPAN: f32 = 30.0;

/// Focus point used when the caller has no player position.
#[inline]
pub fn city_focus(city: &City) -> Vec2 {
    city.spawn_point()
}

/// `true` when a world point is still inside the playable area (a small margin outside
/// the border is tolerated so a car leaving the grid is not instantly deleted).
#[inline]
pub fn inside_city(city: &City, p: Vec2) -> bool {
    let b = city.bounds();
    p.x > b.min.x - DESPAWN_MARGIN
        && p.x < b.max.x + DESPAWN_MARGIN
        && p.y > b.min.y - DESPAWN_MARGIN
        && p.y < b.max.y + DESPAWN_MARGIN
}

/// `true` when an agent at `p` should be recycled: outside the city, or lost beyond the
/// live window.
#[inline]
pub fn should_recycle(city: &City, focus: Vec2, p: Vec2) -> bool {
    !inside_city(city, p) || focus.dist(p) > LIVE_RADIUS
}

/// Candidate sidewalk loops: any loop that has a waypoint inside the respawn annulus
/// `[RESPAWN_RING_NEAR, RESPAWN_RING_MAX]` around `focus`.
pub fn candidate_loops(city: &City, focus: Vec2) -> Vec<usize> {
    let mut out = Vec::new();
    for loop_ in city.loops() {
        if loop_in_band(loop_, focus, RESPAWN_RING_NEAR, RESPAWN_RING_MAX) {
            out.push(loop_.id);
        }
    }
    out
}

/// `true` when any waypoint of `loop_` lies inside the annulus.
pub fn loop_in_band(loop_: &SidewalkLoop, focus: Vec2, lo: f32, hi: f32) -> bool {
    loop_.points()
        .iter()
        .any(|p| focus.dist(*p) >= lo && focus.dist(*p) <= hi)
}

/// Candidate lanes: any lane whose mid point lies inside the respawn annulus.
pub fn candidate_lanes(city: &City, focus: Vec2) -> Vec<usize> {
    let mut out = Vec::new();
    for lane in city.lanes() {
        let mid = lane.start.lerp(lane.end, 0.5);
        let d = focus.dist(mid);
        if d >= RESPAWN_RING_NEAR && d <= RESPAWN_RING_MAX {
            out.push(lane.id);
        }
    }
    out
}
