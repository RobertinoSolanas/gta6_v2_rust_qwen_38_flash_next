//! Input model behaviour: held keys, derived edges, mouse delta consumption, bindings.

use city_input::{action_for_key, InputAction, InputState};

#[test]
fn starts_empty() {
    let i = InputState::new();
    for a in InputAction::ALL {
        assert!(!i.held(a));
        assert!(!i.just_pressed(a));
        assert!(!i.just_released(a));
    }
    assert_eq!(i.move_axis().len(), 0.0);
    assert!(!i.moving());
}

#[test]
fn press_release_edges() {
    let mut i = InputState::new();
    i.press(InputAction::Forward);
    assert!(i.just_pressed(InputAction::Forward));
    assert!(i.held(InputAction::Forward));
    i.end_frame();
    assert!(!i.just_pressed(InputAction::Forward), "edge lasts one frame");
    assert!(i.held(InputAction::Forward));
    i.release(InputAction::Forward);
    assert!(!i.held(InputAction::Forward));
    assert!(i.just_released(InputAction::Forward));
    i.end_frame();
    assert!(!i.just_released(InputAction::Forward));
}

#[test]
fn auto_repeat_does_not_create_a_second_edge() {
    let mut i = InputState::new();
    i.press(InputAction::Jump);
    i.end_frame();
    i.press(InputAction::Jump); // auto repeat
    assert!(i.held(InputAction::Jump));
    assert!(!i.just_pressed(InputAction::Jump));
}

#[test]
fn move_axis_is_normalised_and_signed() {
    let mut i = InputState::new();
    i.press(InputAction::Forward);
    assert_eq!(i.move_axis(), city_math::Vec2::new(0.0, 1.0));
    i.press(InputAction::Right);
    let diag = i.move_axis();
    assert!(
        (diag.len() - 1.0).abs() < 1e-5,
        "diagonal must not be faster: {}",
        diag.len()
    );
    assert!(diag.x > 0.0 && diag.y > 0.0);
    i.release(InputAction::Forward);
    i.release(InputAction::Right);
    i.press(InputAction::Back);
    i.press(InputAction::Left);
    let back = i.move_axis();
    assert!(back.x < 0.0 && back.y < 0.0);
    // Back + Forward cancel out completely (`Left` is still held, so clear it first).
    i.release(InputAction::Left);
    i.press(InputAction::Forward);
    assert_eq!(
        i.move_axis(),
        city_math::Vec2::ZERO,
        "opposing keys must cancel"
    );
}

#[test]
fn moving_flag_tracks_the_axis() {
    let mut i = InputState::new();
    assert!(!i.moving());
    i.press(InputAction::Left);
    assert!(i.moving());
    i.release(InputAction::Left);
    assert!(!i.moving());
}

#[test]
fn look_delta_is_consumed_once() {
    let mut i = InputState::new();
    i.add_look(12.0, -3.0);
    i.add_look(2.0, 1.0);
    let (dx, dy) = i.take_look();
    assert_eq!((dx, dy), (14.0, -2.0));
    assert_eq!(i.take_look(), (0.0, 0.0));
}

#[test]
fn look_ignores_non_finite_deltas() {
    let mut i = InputState::new();
    i.add_look(f32::NAN, f32::NAN);
    assert_eq!(i.take_look(), (0.0, 0.0));
    i.add_look(5.0, f32::NAN);
    let (dx, dy) = i.take_look();
    assert_eq!(dx, 5.0);
    assert_eq!(dy, 0.0);
}

#[test]
fn wheel_is_consumed_once() {
    let mut i = InputState::new();
    i.add_wheel(1.0);
    i.add_wheel(-0.5);
    assert_eq!(i.take_wheel(), 0.5);
    assert_eq!(i.take_wheel(), 0.0);
}

#[test]
fn release_all_clears_everything() {
    let mut i = InputState::new();
    for a in InputAction::ALL {
        i.press(a);
    }
    i.add_look(10.0, 10.0);
    i.add_wheel(2.0);
    i.pointer_locked = true;
    i.release_all();
    for a in InputAction::ALL {
        assert!(!i.held(a), "{a:?} stuck down");
    }
    assert_eq!(i.held_count(), 0);
    assert_eq!(i.take_look(), (0.0, 0.0));
    assert_eq!(i.take_wheel(), 0.0);
    assert!(!i.pointer_locked);
}

#[test]
fn held_count_tracks_keys() {
    let mut i = InputState::new();
    i.press(InputAction::Forward);
    i.press(InputAction::Sprint);
    assert_eq!(i.held_count(), 2);
    i.release(InputAction::Forward);
    assert_eq!(i.held_count(), 1);
}

#[test]
fn key_bindings_cover_wasd_and_hotkeys() {
    assert_eq!(action_for_key("w"), Some(InputAction::Forward));
    assert_eq!(action_for_key("S"), Some(InputAction::Back));
    assert_eq!(action_for_key("a"), Some(InputAction::Left));
    assert_eq!(action_for_key("D"), Some(InputAction::Right));
    assert_eq!(action_for_key("Shift"), Some(InputAction::Sprint));
    assert_eq!(action_for_key(" "), Some(InputAction::Jump));
    assert_eq!(action_for_key("f"), Some(InputAction::CycleCamera));
    assert_eq!(action_for_key("T"), Some(InputAction::TimeSkip));
    assert_eq!(action_for_key("h"), Some(InputAction::ToggleHud));
    assert_eq!(action_for_key("q"), None);
    assert_eq!(action_for_key(""), None);
    // arrows are aliases of the WASD actions
    assert_eq!(action_for_key("ArrowUp"), action_for_key("w"));
    assert_eq!(action_for_key("ArrowLeft"), action_for_key("a"));
}

#[test]
fn every_action_has_a_unique_bit() {
    let mut seen = 0u32;
    for a in InputAction::ALL {
        let b = 1u32 << a.index();
        assert_eq!(seen & b, 0, "duplicate bit for {a:?}");
        seen |= b;
    }
    assert_eq!(seen.count_ones(), InputAction::ALL.len() as u32);
}

#[test]
fn yaw_wrap_helper_wraps_look_deltas() {
    use city_math::PI;
    assert!((InputState::wrap_yaw(0.0)).abs() < 1e-6);
    assert!((InputState::wrap_yaw(PI * 2.0) - 0.0).abs() < 1e-5);
    assert!(InputState::wrap_yaw(-PI * 3.0).abs() <= PI + 1e-6);
}
