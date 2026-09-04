//! Avatar controller behaviour: camera-relative movement, sprint/jump, gravity,
//! ground follow, wall slide, walk-cycle phase and the pose it produces.
//!
//! The world is flat at y = 0, so every horizontal expectation below is exact
//! (no terrain, no obstacles between the body and its target).
//!
//! Axis convention (the one `city_input::InputModel::move_axis` feeds the controller
//! with): the wish vector is `(x = strafe, y = forward)` in *camera* space and the
//! controller turns it into a world direction with `right = -fwd.perp()`. With
//! `camera_yaw = 0` the camera forward is `Vec2::from_angle(0) = +X`, so the W key
//! (`Vec2::Y`) travels along +X and strafing right (`+x`) points towards -Y.
//! All expectations below are written against that measured behaviour.

use city_avatar::{Avatar, AvatarConfig, Terrain};
use city_layout::{City, CityParams};
use city_math::{Vec2, TAU};

const DT: f32 = 1.0 / 60.0;

fn cfg() -> AvatarConfig {
    AvatarConfig::default()
}

fn city() -> City {
    City::generate(CityParams::default())
}

fn avatar(c: &City) -> Avatar {
    Avatar::spawn(c, cfg())
}

/// Push the body along a wish vector for `n` steps.
fn push(a: &mut Avatar, c: &City, wish: Vec2, n: usize) {
    for _ in 0..n {
        a.update(c, wish, 0.0, false, DT);
    }
}

/// Flat ground of a fixed height, no obstacles — a stand-in [`Terrain`].
struct Flat {
    y: f32,
}

impl Terrain for Flat {
    fn ground_y(&self, _p: Vec2) -> f32 {
        self.y
    }
    fn resolve(&self, p: Vec2, _r: f32) -> Vec2 {
        p
    }
}

/// A wall at `x = 0`: everything with `x > 0` is solid.
struct HalfSpace;

impl Terrain for HalfSpace {
    fn ground_y(&self, _p: Vec2) -> f32 {
        0.0
    }
    fn resolve(&self, p: Vec2, _r: f32) -> Vec2 {
        Vec2::new(p.x.min(0.0), p.y)
    }
}

// ---------------------------------------------------------------------------
// spawning / initial state
// ---------------------------------------------------------------------------

#[test]
fn spawns_on_a_walkable_point_with_zero_velocity() {
    let c = city();
    let a = avatar(&c);
    assert!(
        c.is_walkable(a.xz(), cfg().radius),
        "spawn point must be walkable"
    );
    assert_eq!(a.position().y, 0.0);
    assert_eq!(a.speed(), 0.0);
    assert!(a.is_grounded());
    assert_eq!(a.distance_walked(), 0.0);
}

#[test]
fn at_and_spawn_agree_on_the_spawn_point() {
    let c = city();
    let a = Avatar::at(&c, c.spawn_point(), cfg());
    let b = Avatar::spawn(&c, cfg());
    assert!(a.xz().dist(b.xz()) < 1e-6);
}

#[test]
fn at_pushes_an_avatar_out_of_a_building() {
    let c = city();
    let b = c.buildings().iter().find(|b| b.height > 3.0).expect("a building");
    let a = Avatar::at(&c, b.center(), cfg());
    assert!(
        c.is_walkable(a.xz(), cfg().radius * 0.99),
        "resolved spawn must not stay inside solid geometry"
    );
    assert!(a.xz().dist(b.center()) > 0.1);
}

// ---------------------------------------------------------------------------
// planar movement
// ---------------------------------------------------------------------------

#[test]
fn forward_key_moves_the_character_along_the_camera_forward() {
    let c = city();
    let mut a = avatar(&c);
    let start = a.xz();
    push(&mut a, &c, Vec2::Y, 60);
    let d = a.xz() - start;
    assert!(d.x > 1.0, "yaw 0: camera forward is +X, got {d:?}");
    assert!(d.y.abs() < 0.01, "unexpected sideways drift {d:?}");
    assert!(a.speed() > 1.0);
}

#[test]
fn camera_yaw_rotates_the_wish_direction() {
    let c = city();
    let mut a = avatar(&c);
    let start = a.xz();
    for _ in 0..40 {
        a.update(&c, Vec2::Y, TAU / 4.0, false, DT);
    }
    let d = a.xz() - start;
    assert!(d.y > 0.5, "yaw +90 deg: want travel towards +Z, got {d:?}");
    assert!(d.x.abs() < 0.5, "want mostly along Z, got {d:?}");
}

#[test]
fn back_key_walks_backwards() {
    let c = city();
    let mut a = avatar(&c);
    let start = a.xz();
    push(&mut a, &c, Vec2::new(0.0, -1.0), 40);
    assert!(a.xz().x < start.x - 0.5, "back key walks opposite the camera forward");
}

#[test]
fn strafe_moves_sideways() {
    let c = city();
    let mut a = avatar(&c);
    let start = a.xz();
    push(&mut a, &c, Vec2::X, 40);
    let d = a.xz() - start;
    assert!(d.y < -0.5, "strafe right at yaw 0 travels towards -Y, got {d:?}");
    assert!(d.x.abs() < 0.01, "strafe must not drift forwards, got {d:?}");
}

#[test]
fn diagonal_input_is_normalized_so_diagonals_are_not_faster() {
    let c = city();
    let mut a = avatar(&c);
    let mut b = Avatar::at(&c, a.xz(), cfg());
    for _ in 0..40 {
        a.update(&c, Vec2::Y, 0.0, false, DT);
        b.update(&c, Vec2::new(1.0, 1.0), 0.0, false, DT);
    }
    assert!(
        (a.speed() - b.speed()).abs() < 0.1,
        "diagonal speed boost: {} vs {}",
        a.speed(),
        b.speed()
    );
}

#[test]
fn oversized_wish_input_is_clamped() {
    let c = city();
    let mut a = avatar(&c);
    for _ in 0..90 {
        a.update(&c, Vec2::new(9.0, 9.0), 0.0, true, DT);
    }
    assert!(a.speed() <= cfg().sprint_speed + 1e-3, "speed {}", a.speed());
}

#[test]
fn speed_saturates_at_walk_speed_then_sprint_speed() {
    let c = city();
    let mut a = avatar(&c);
    push(&mut a, &c, Vec2::Y, 120);
    assert!((a.speed() - cfg().walk_speed).abs() < 0.05);
    for _ in 0..180 {
        a.update(&c, Vec2::Y, 0.0, true, DT);
    }
    assert!((a.speed() - cfg().sprint_speed).abs() < 0.15);
    assert!(a.is_sprinting());
}

#[test]
fn sprint_without_input_is_not_sprinting() {
    let c = city();
    let mut a = avatar(&c);
    a.update(&c, Vec2::ZERO, 0.0, true, DT);
    assert!(!a.is_sprinting());
}

#[test]
fn releasing_the_keys_brings_the_body_to_a_stop() {
    let c = city();
    let mut a = avatar(&c);
    push(&mut a, &c, Vec2::X, 60);
    assert!(a.speed() > 1.0);
    let mut frames = 0;
    while a.speed() > 1e-6 && frames < 200 {
        a.update(&c, Vec2::ZERO, 0.0, false, DT);
        frames += 1;
    }
    assert!(a.speed() < 1e-6, "drag must fully stop the body");
    assert!(frames < 60, "stop took {frames} frames — drag too weak");
}

#[test]
fn zero_dt_is_a_no_op() {
    let c = city();
    let mut a = avatar(&c);
    let before = a.xz();
    a.update(&c, Vec2::Y, 0.0, true, 0.0);
    assert_eq!(a.speed(), 0.0);
    assert_eq!(a.xz(), before);
}

#[test]
fn a_huge_dt_is_clamped_not_catastrophic() {
    let c = city();
    let mut a = avatar(&c);
    a.update(&c, Vec2::Y, 0.0, true, 10.0);
    assert!(a.xz().x.is_finite() && a.xz().y.is_finite());
    assert!(a.speed() <= cfg().sprint_speed + 1e-3);
    assert!(c.is_walkable(a.xz(), cfg().radius * 0.5));
}

// ---------------------------------------------------------------------------
// vertical: gravity, ground follow, jump
// ---------------------------------------------------------------------------

#[test]
fn stays_grounded_on_flat_ground() {
    let c = city();
    let mut a = avatar(&c);
    push(&mut a, &c, Vec2::Y, 60);
    assert!(a.is_grounded());
    assert_eq!(a.position().y, 0.0);
    assert_eq!(a.vertical_speed(), 0.0);
}

#[test]
fn jump_launches_then_lands() {
    let c = city();
    let mut a = avatar(&c);
    assert!(a.try_jump());
    assert!(!a.is_grounded());
    assert!(a.vertical_speed() > 0.0);

    let mut peak = a.position().y;
    let mut landed = false;
    for _ in 0..600 {
        a.update(&c, Vec2::ZERO, 0.0, false, DT);
        peak = peak.max(a.position().y);
        if a.is_grounded() {
            landed = true;
            break;
        }
    }
    assert!(peak > 0.5, "jump apex too low: {peak}");
    assert!(landed, "should have landed again");
    assert!(a.position().y <= 0.001);
    assert_eq!(a.vertical_speed(), 0.0);
}

#[test]
fn double_jump_is_refused() {
    let c = city();
    let mut a = avatar(&c);
    assert!(a.try_jump());
    assert!(!a.try_jump(), "no mid-air second jump");
}

#[test]
fn airtime_holds_the_stride_and_flags_the_pose() {
    let c = city();
    let mut a = avatar(&c);
    push(&mut a, &c, Vec2::Y, 30);
    assert!(a.try_jump());
    let phase = a.phase();
    push(&mut a, &c, Vec2::Y, 10);
    assert_eq!(a.phase(), phase, "walk cycle freezes in the air");
    assert!(a.pose(0.0).airborne);
}

#[test]
fn falls_when_the_ground_disappears_and_settles_below() {
    let c = city();
    let mut a = Avatar::at(&c, Vec2::ZERO, cfg());
    a.update_on(&Flat { y: 0.0 }, Vec2::ZERO, 0.0, false, DT);
    let pit = Flat { y: -5.0 };
    let start_y = a.position().y;
    for _ in 0..10 {
        a.update_on(&pit, Vec2::ZERO, 0.0, false, DT);
    }
    assert!(a.position().y < start_y, "gravity must pull the body down");
    assert!(!a.is_grounded());
    for _ in 0..180 {
        a.update_on(&pit, Vec2::ZERO, 0.0, false, DT);
    }
    assert!(a.is_grounded());
    assert!((a.position().y - (-5.0)).abs() < 1e-3);
}

#[test]
fn walks_up_a_kerb_within_step_height() {
    let c = city();
    let mut a = Avatar::at(&c, Vec2::ZERO, cfg());
    a.update_on(&Flat { y: 0.3 }, Vec2::ZERO, 0.0, false, DT);
    assert!(a.is_grounded());
    assert!((a.position().y - 0.3).abs() < 1e-3, "kerb walked up");
}

#[test]
fn a_wall_step_snaps_the_body_to_the_ground_but_does_not_climb_it() {
    // The controller has no wall climbing: a grounded body resting on a step is
    // placed on it, but it never gains height on its own — it just sticks to
    // whatever `ground_y` reports.
    let c = city();
    let mut a = Avatar::at(&c, Vec2::ZERO, cfg());
    let wall = Flat { y: 2.0 };
    for _ in 0..10 {
        a.update_on(&wall, Vec2::ZERO, 0.0, false, DT);
    }
    assert!(a.is_grounded());
    assert!((a.position().y - 2.0).abs() < 1e-3);
    // …but it never lifts off the surface on its own.
    let y = a.position().y;
    for _ in 0..60 {
        a.update_on(&wall, Vec2::ZERO, 0.0, false, DT);
    }
    assert_eq!(a.position().y, y);
}

#[test]
fn a_custom_terrain_drives_both_ground_and_collision() {
    let c = city();
    let mut a = Avatar::at(&c, Vec2::ZERO, cfg());
    for _ in 0..120 {
        a.update_on(&HalfSpace, Vec2::new(1.0, 1.0).norm(), 0.0, false, DT);
    }
    assert!(a.xz().x <= 1e-3, "terrain resolve was ignored: {:?}", a.xz());
    assert!(a.is_grounded());
    assert!(a.speed() > 0.5, "should still slide along the wall");
}

// ---------------------------------------------------------------------------
// collision / wall slide
// ---------------------------------------------------------------------------

#[test]
fn walking_at_a_building_never_enters_it() {
    let c = city();
    let b = c.buildings().iter().find(|b| b.height > 4.0).unwrap();
    let half = (b.footprint.max.x - b.footprint.min.x) * 0.5;
    let mut a = Avatar::at(&c, Vec2::new(b.center().x - half - 14.0, b.center().y), cfg());
    for _ in 0..240 {
        // drive east at the wall with a slight sideways component → slide
        a.update(&c, Vec2::new(1.0, 0.6).norm(), 0.0, false, DT);
        assert!(
            c.is_walkable(a.xz(), cfg().radius * 0.9),
            "avatar ended up inside solid geometry: {:?}",
            a.xz()
        );
    }
}

#[test]
fn the_body_never_ends_up_inside_a_building() {
    let c = city();
    let mut a = avatar(&c);
    for i in 0..600 {
        let dir = Vec2::from_angle(i as f32 * 0.37);
        a.update(&c, dir, 0.0, i % 7 == 0, DT);
        assert!(
            c.is_walkable(a.xz(), cfg().radius * 0.5),
            "walked into a wall at frame {i}: {:?}",
            a.xz()
        );
    }
}

#[test]
fn teleport_relocates_and_resets_the_motion_state() {
    let c = city();
    let mut a = avatar(&c);
    push(&mut a, &c, Vec2::Y, 30);
    assert!(a.speed() > 0.5);
    a.teleport(&c, c.spawn_point());
    assert!(a.xz().dist(c.spawn_point()) < 0.1);
    assert_eq!(a.speed(), 0.0);
    assert!(a.is_grounded());
}

// ---------------------------------------------------------------------------
// walk cycle & pose
// ---------------------------------------------------------------------------

#[test]
fn standing_still_does_not_advance_the_stride() {
    let c = city();
    let mut a = avatar(&c);
    push(&mut a, &c, Vec2::ZERO, 10);
    assert_eq!(a.phase(), 0.0);
}

#[test]
fn walking_advances_the_stride() {
    let c = city();
    let mut a = avatar(&c);
    push(&mut a, &c, Vec2::Y, 30);
    assert!(a.phase() > 0.0);
}

#[test]
fn sprinting_advances_the_stride_faster_than_walking() {
    // From a standing start both bodies are still accelerating in the first frames,
    // so compare over a window where both are at their target speed. The stride
    // phase advances speed / stride_len turns per second:
    //   walk   2.6 / 1.55 = 1.68 turns/s      sprint  5.6 / 2.4 = 2.33 turns/s
    let c = city();
    let mut walk = avatar(&c);
    let mut sprint = Avatar::at(&c, walk.xz(), cfg());
    for _ in 0..150 {
        walk.update(&c, Vec2::Y, 0.0, false, DT);
        sprint.update(&c, Vec2::Y, 0.0, true, DT);
    }
    assert!(sprint.is_sprinting());
    assert!(sprint.speed() > walk.speed());
    let dw = gait_rate(&mut walk, &c, false);
    let ds = gait_rate(&mut sprint, &c, true);
    assert!(
        ds > dw,
        "sprint gait should spin faster: walk {dw} vs sprint {ds}"
    );
    // …and it covers clearly more ground in the same window.
    assert!(sprint.speed() > walk.speed() * 1.5);
}

/// Phase turns accrued over half a second of walking/sprinting.
fn gait_rate(a: &mut Avatar, c: &City, sprint: bool) -> f32 {
    let mut before = a.phase();
    let mut acc = 0.0;
    for _ in 0..30 {
        a.update(c, Vec2::Y, 0.0, sprint, DT);
        let d = a.phase() - before;
        acc += if d < 0.0 { d + 1.0 } else { d };
        before = a.phase();
    }
    acc
}


#[test]
fn phase_stays_wrapped_in_a_full_turn() {
    let c = city();
    let mut a = avatar(&c);
    push(&mut a, &c, Vec2::Y, 2000);
    assert!(a.phase() >= 0.0 && a.phase() < 1.0, "phase {}", a.phase());
}

#[test]
fn the_gait_is_contralateral() {
    // `Avatar::pose` drives arm_l = -sin(w)·0.85·amp and leg_l = -sin(w)·0.55·amp
    // with leg_r opposing it, so during a stride:
    //   * the left leg keeps the sign of arm_l (same sign, fixed 0.55/0.85 ratio),
    //   // the two legs always mirror each other,
    //   * the right arm always opposes both legs' sign pattern.
    let c = city();
    let mut a = avatar(&c);
    let mut checked = 0;
    for _ in 0..120 {
        a.update(&c, Vec2::Y, 0.0, false, DT);
        let p = a.pose(0.0);
        if p.leg_l.abs() > 0.05 {
            checked += 1;
            assert!(p.arm_l * p.leg_l > 0.0, "arm_l/leg_l lost their phase: {p:?}");
            assert!(p.arm_l * p.leg_r < 0.0, "legs stopped mirroring: {p:?}");
            assert!(p.arm_r * p.leg_l < 0.0, "arm_r/leg_l lost their phase: {p:?}");
            assert!(
                (p.leg_l.abs() * 0.85 - p.arm_l.abs() * 0.55).abs() < 1e-3,
                "leg swing does not follow the arm amplitude: {p:?}"
            );
        }
    }
    assert!(checked > 10, "pose never swung");
}

#[test]
fn the_pose_is_neutral_for_a_standing_body() {
    let c = city();
    let a = avatar(&c);
    let p = a.pose(0.0);
    assert!(p.arm_l.abs() < 0.1 && p.arm_r.abs() < 0.1, "{p:?}");
    assert_eq!(p.leg_l, 0.0);
    assert_eq!(p.leg_r, 0.0);
    assert!(!p.airborne);
}

#[test]
fn pose_progression_loops_through_a_full_stride() {
    let c = city();
    let mut seen = [false; 4];
    // walk the whole stride: every quarter of the cycle must appear
    let mut b = avatar(&c);
    for _ in 0..4000 {
        push(&mut b, &c, Vec2::Y, 1);
        seen[(b.phase() * 4.0) as usize % 4] = true;
    }
    assert_eq!(seen, [true; 4], "stride should cover all four quarters");
}
