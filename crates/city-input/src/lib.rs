//! # city-input
//!
//! Bounded context for *what the player asked for*. It knows nothing about the DOM: the
//! browser glue in `city-app` turns key/mouse events into [`InputAction`] presses and
//! mouse deltas, everything downstream (avatar, camera, HUD) reads [`InputState`].
//!
//! Design notes:
//! * Movement is a *held* state; camera look is a delta consumed exactly once per frame
//!   by the camera rig ([`InputState::take_look`]).
//! * Edges (`just_pressed`) are derived from the previous frame's bit set, so they can
//!   never drift out of sync with the held keys.
//! * Losing focus must never leave a key stuck down — see [`InputState::release_all`].

#![forbid(unsafe_code)]

use city_math::wrap_angle;

/// Everything the player can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputAction {
    Forward,
    Back,
    Left,
    Right,
    Sprint,
    Jump,
    /// Cycle the third-person camera distance (`F`).
    CycleCamera,
    /// Skip to the next phase of the day (`T`).
    TimeSkip,
    /// Hide / show the HUD (`H`).
    ToggleHud,
}

impl InputAction {
    /// Every action (order matches [`InputAction::index`]).
    pub const ALL: [InputAction; 9] = [
        InputAction::Forward,
        InputAction::Back,
        InputAction::Left,
        InputAction::Right,
        InputAction::Sprint,
        InputAction::Jump,
        InputAction::CycleCamera,
        InputAction::TimeSkip,
        InputAction::ToggleHud,
    ];

    /// Dense index used as a bit position.
    #[inline]
    pub const fn index(self) -> u32 {
        match self {
            InputAction::Forward => 0,
            InputAction::Back => 1,
            InputAction::Left => 2,
            InputAction::Right => 3,
            InputAction::Sprint => 4,
            InputAction::Jump => 5,
            InputAction::CycleCamera => 6,
            InputAction::TimeSkip => 7,
            InputAction::ToggleHud => 8,
        }
    }
}

/// Map a DOM `KeyboardEvent.key` to an action (`None` = not bound).
pub fn action_for_key(key: &str) -> Option<InputAction> {
    match key {
        "w" | "W" | "ArrowUp" => Some(InputAction::Forward),
        "s" | "S" | "ArrowDown" => Some(InputAction::Back),
        "a" | "A" | "ArrowLeft" => Some(InputAction::Left),
        "d" | "D" | "ArrowRight" => Some(InputAction::Right),
        "Shift" => Some(InputAction::Sprint),
        " " | "Spacebar" => Some(InputAction::Jump),
        "f" | "F" => Some(InputAction::CycleCamera),
        "t" | "T" => Some(InputAction::TimeSkip),
        "h" | "H" => Some(InputAction::ToggleHud),
        _ => None,
    }
}

/// Held keys and accumulated mouse input for one frame.
#[derive(Clone, Debug)]
pub struct InputState {
    down: u32,
    prev: u32,
    dx: f32,
    dy: f32,
    wheel: f32,
    /// `true` while the browser has captured the pointer.
    pub pointer_locked: bool,
}

impl Default for InputState {
    fn default() -> Self {
        InputState::new()
    }
}

impl InputState {
    /// Nothing pressed.
    pub fn new() -> InputState {
        InputState {
            down: 0,
            prev: 0,
            dx: 0.0,
            dy: 0.0,
            wheel: 0.0,
            pointer_locked: false,
        }
    }

    #[inline]
    fn mask(a: InputAction) -> u32 {
        1u32 << a.index()
    }

    /// Register a key-down (`repeat` events count as already-held).
    pub fn press(&mut self, a: InputAction) {
        self.down |= Self::mask(a);
    }

    /// Register a key-up.
    pub fn release(&mut self, a: InputAction) {
        self.down &= !Self::mask(a);
    }

    /// Accumulate raw mouse movement (pixels) for the next frame.
    pub fn add_look(&mut self, dx: f32, dy: f32) {
        if dx.is_finite() {
            self.dx += dx;
        }
        if dy.is_finite() {
            self.dy += dy;
        }
    }

    /// Accumulate wheel ticks (already normalised by the caller).
    pub fn add_wheel(&mut self, ticks: f32) {
        if ticks.is_finite() {
            self.wheel += ticks;
        }
    }

    /// `true` while `a` is held.
    #[inline]
    pub fn held(&self, a: InputAction) -> bool {
        self.down & Self::mask(a) != 0
    }

    /// `true` only on the frame `a` went down.
    #[inline]
    pub fn just_pressed(&self, a: InputAction) -> bool {
        let m = Self::mask(a);
        self.down & m != 0 && self.prev & m == 0
    }

    /// `true` on the frame `a` was released.
    #[inline]
    pub fn just_released(&self, a: InputAction) -> bool {
        let m = Self::mask(a);
        self.down & m == 0 && self.prev & m != 0
    }

    /// Camera-relative move wish: `(right, forward)`, each `-1..=1`, length clamped.
    ///
    /// *move_scale* is `1.0` for walk speed, `>1` to sprint — the caller decides, this
    /// only reports direction and whether `Sprint` is held.
    pub fn move_axis(&self) -> city_math::Vec2 {
        let mut x = 0.0f32;
        let mut z = 0.0f32;
        if self.held(InputAction::Right) {
            x += 1.0;
        }
        if self.held(InputAction::Left) {
            x -= 1.0;
        }
        if self.held(InputAction::Forward) {
            z += 1.0;
        }
        if self.held(InputAction::Back) {
            z -= 1.0;
        }
        city_math::Vec2::new(x, z).clamp_len(1.0)
    }

    /// `true` when the player wants to move at all.
    #[inline]
    pub fn moving(&self) -> bool {
        self.move_axis().len_sq() > 1e-6
    }

    /// Consume the accumulated look delta (and reset it).
    pub fn take_look(&mut self) -> (f32, f32) {
        let d = (self.dx, self.dy);
        self.dx = 0.0;
        self.dy = 0.0;
        d
    }

    /// Consume the accumulated wheel ticks.
    pub fn take_wheel(&mut self) -> f32 {
        let w = self.wheel;
        self.wheel = 0.0;
        w
    }

    /// Drop every held key (focus loss, pointer unlock, dialog takeover).
    pub fn release_all(&mut self) {
        self.down = 0;
        self.prev = 0;
        self.dx = 0.0;
        self.dy = 0.0;
        self.wheel = 0.0;
        self.pointer_locked = false;
    }

    /// Number of held actions (debug/HUD).
    #[inline]
    pub fn held_count(&self) -> u32 {
        self.down.count_ones()
    }

    /// Mark the end of a frame: `just_pressed` edges are only valid *before* this call.
    pub fn end_frame(&mut self) {
        self.prev = self.down;
    }

    /// Wrap a yaw delta into `(-PI, PI]` — the camera uses this on `take_look`.
    #[inline]
    pub fn wrap_yaw(a: f32) -> f32 {
        wrap_angle(a)
    }
}
