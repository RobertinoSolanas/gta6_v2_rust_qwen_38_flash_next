//! The pure world (`World`): generation, fixed stepping, avatar + camera wiring and HUD
/// output. These run natively, so a browser bug and a logic bug stay distinguishable.

use city_app::{World, WorldConfig};

fn world() -> World {
    World::new(WorldConfig::default())
}

#[test]
fn boot_generates_a_world() {
    let w = world();
    assert!(w.city().buildings().len() > 50);
    assert!(w.city().props().len() > 100);
    assert!(w.avatar().is_grounded());
    assert_eq!(w.frames(), 0);
}

#[test]
fn spawn_is_walkable() {
    let w = world();
    assert!(w.city().is_walkable(w.spawn(), 0.5));
    assert!(w.city().is_walkable(w.avatar().xz(), 0.4));
}

#[test]
fn tick_advances_the_clock() {
    let mut w = world();
    w.tick(1.0);
    assert!(w.frames() >= 1);
    assert!(w.hours() > 8.0, "time moves forward");
}

#[test]
fn walking_moves_the_avatar() {
    let mut w = world();
    let start = w.avatar().xz();
    w.key("w", true);
    for _ in 0..60 {
        w.step(1.0 / 60.0);
    }
    let moved = w.avatar().xz();
    assert!(
        moved.dist(start) > 2.0,
        "one second of walking should cover ~2.6 m, got {:.2}",
        moved.dist(start)
    );
    assert!(w.avatar().speed() > 1.5);
    assert!(w.avatar().distance_walked() > 1.5);
}

#[test]
fn sprint_is_faster_than_walk() {
    let mut walk = world();
    walk.key("w", true);
    for _ in 0..120 {
        walk.step(1.0 / 60.0);
    }
    let walk_speed = walk.avatar().speed();

    let mut sprint = world();
    sprint.key("w", true);
    sprint.key("Shift", true);
    for _ in 0..120 {
        sprint.step(1.0 / 60.0);
    }
    let sprint_speed = sprint.avatar().speed();
    assert!(sprint.avatar().is_sprinting());
    assert!(
        sprint_speed > walk_speed * 1.4,
        "walk {walk_speed} sprint {sprint_speed}"
    );
}

#[test]
fn releasing_the_key_brings_the_player_to_a_stop() {
    let mut w = world();
    w.key("w", true);
    for _ in 0..30 {
        w.step(1.0 / 60.0);
    }
    w.key("w", false);
    for _ in 0..60 {
        w.step(1.0 / 60.0);
    }
    assert!(w.avatar().speed() < 0.05);
    assert!(w.avatar().is_grounded());
}

#[test]
fn jumping_leaves_the_ground_and_lands() {
    let mut w = world();
    w.key(" ", true);
    w.step(1.0 / 60.0);
    assert!(!w.avatar().is_grounded(), "should be airborne");
    assert!(w.avatar().vertical_speed() > 0.0);
    for _ in 0..120 {
        w.step(1.0 / 60.0);
    }
    assert!(w.avatar().is_grounded(), "must come back down");
}

#[test]
fn camera_follows_the_avatar() {
    let mut w = world();
    let start = w.avatar().xz();
    w.key("w", true);
    for _ in 0..120 {
        w.step(1.0 / 60.0);
    }
    let focus = w.camera().focus();
    assert!((focus.x - start.x).abs() + (focus.y - start.y).abs() > 1.0);
    let eye = w.camera().eye();
    let d = ((focus.x - eye.x).powi(2) + (focus.z - eye.z).powi(2)).sqrt();
    assert!(d > 1.0 && d < 20.0, "boom length {d}");
}

#[test]
fn mouse_look_only_works_while_locked() {
    let mut w = world();
    let yaw0 = w.camera().yaw();
    w.mouse(200.0, 0.0);
    w.step(1.0 / 60.0);
    assert!((yaw0 - w.camera().yaw()).abs() < 1e-6, "unlocked: no look");

    w.set_pointer_locked(true);
    w.mouse(200.0, 0.0);
    w.step(1.0 / 60.0);
    assert!(
        (yaw0 - w.camera().yaw()).abs() > 0.1,
        "locked look must turn the camera"
    );
}

#[test]
fn time_flows_and_the_clock_formats() {
    let mut w = world();
    w.set_hours(11.0);
    assert_eq!(w.sample().phase(), "day");
    let before = w.hours();
    w.tick(60.0);
    assert!(w.hours() > before);
    assert_eq!(w.clock().clock().len(), 5, "HH:MM");
}

#[test]
fn time_skip_lands_on_noon() {
    let mut w = world();
    w.set_hours(9.0);
    w.time_skip();
    assert!(w.clock().is_skipping());
    // the skip animation takes SKIP_SECONDS (1.5 s) of real time
    let mut guard = 0;
    while w.clock().is_skipping() && guard < 400 {
        w.tick(1.0 / 60.0);
        guard += 1;
    }
    assert!(
        (w.hours() - 12.0).abs() < 0.05,
        "skip should land on noon, got {}",
        w.hours()
    );
    assert!(!w.clock().is_skipping());
}

#[test]
fn night_lights_come_on_after_dark() {
    let day = world();
    let day_sky = day.sample();
    assert!(day_lamp(day_sky) < 0.02);

    let mut night = world();
    night.set_hours(22.0);
    let s = night.sample();
    assert!(s.lamp_light > 0.9);
    assert!(s.window_light > 0.9);
    assert!(s.exposure > 1.5);
}

/// Helper: the lamp curve of a sample (kept explicit for readability).
fn day_lamp(s: city_sky::SkySample) -> f32 {
    s.lamp_light
}

#[test]
fn hud_is_empty_when_hidden() {
    let mut w = world();
    let full = w.hud_frame();
    assert!(!full.clock.is_empty());
    assert!(full.lines.len() > 4, "radar must contain streets");
    assert!(full
        .dots
        .iter()
        .any(|d| d.kind == city_hud::HudDotKind::Player));

    w.set_hud_visible(false);
    let hidden = w.hud_frame();
    assert!(hidden.lines.is_empty());
    assert!(hidden.dots.is_empty());
    assert!(!hidden.clock.is_empty(), "the clock is always available");
}

#[test]
fn hotkeys_work_through_key_events() {
    let mut w = world();
    let cam0 = w.camera().distance_index();
    w.key("f", true);
    w.step(1.0 / 60.0);
    assert_eq!(w.camera().distance_index(), (cam0 + 1) % 4);

    w.key("h", true);
    w.step(1.0 / 60.0);
    assert!(!w.hud_visible());

    w.key("t", true);
    w.step(1.0 / 60.0);
    assert!(w.clock().is_skipping());
}

#[test]
fn unbound_keys_are_ignored() {
    let mut w = world();
    w.key("q", true);
    w.key("Escape", true);
    w.step(1.0 / 60.0);
    assert_eq!(w.input().held_count(), 0);
}

#[test]
fn losing_focus_stops_the_character() {
    let mut w = world();
    w.key("w", true);
    for _ in 0..30 {
        w.step(1.0 / 60.0);
    }
    assert!(w.avatar().speed() > 0.5);
    w.set_pointer_locked(false);
    for _ in 0..60 {
        w.step(1.0 / 60.0);
    }
    assert!(w.avatar().speed() < 0.05);
}

#[test]
fn snapshot_is_json_and_complete() {
    let mut w = world();
    w.tick(0.5);
    let json = w.snapshot_json();
    assert!(json.starts_with('{') && json.ends_with('}'));
    for key in [
        "\"clock\"",
        "\"phase\"",
        "\"player_x\"",
        "\"speed_kmh\"",
        "\"cam_index\"",
        "\"buildings\"",
        "\"tip\"",
    ] {
        assert!(json.contains(key), "missing {key} in {json}");
    }
}

#[test]
fn stepping_is_deterministic() {
    let run = || {
        let mut w = World::new(WorldConfig::default());
        w.key("w", true);
        for _ in 0..240 {
            w.step(1.0 / 60.0);
        }
        w.snapshot_json()
    };
    assert_eq!(run(), run());
}

#[test]
fn config_controls_start_time_and_grid() {
    let mut cfg = WorldConfig::default();
    cfg.start_hours = 22.0;
    cfg.params = city_layout::CityParams {
        blocks_x: 9,
        blocks_z: 5,
        ..city_layout::CityParams::default()
    };
    let w = World::new(cfg);
    assert_eq!(w.city().grid(), [9usize, 5usize]);
    assert!((w.hours() - 22.0).abs() < 0.01);
}

#[test]
fn absurd_deltas_cannot_break_the_world() {
    let mut w = world();
    w.tick(30.0);
    w.tick(f32::NAN);
    w.tick(-4.0);
    assert!(w.elapsed() < 31.0);
    assert!(w.avatar().is_grounded());
    assert!(w.avatar().speed().is_finite());
    assert!(w.sample().exposure.is_finite());
}

#[test]
fn teleport_puts_the_avatar_back_on_the_ground() {
    let mut w = world();
    let p = w.spawn();
    w.teleport(city_math::Vec2::new(p.x + 12.0, p.y));
    assert!(w.avatar().is_grounded());
    assert!(w.avatar().speed() < 0.01);
    assert!(w.city().is_walkable(w.avatar().xz(), 0.4));
}
