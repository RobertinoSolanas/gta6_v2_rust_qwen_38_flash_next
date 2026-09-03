//! The pure world: city + day/night clock + avatar + camera + HUD model.
//!
//! Everything here runs natively too (`cargo test -p city-app`), so a browser bug and a
//! logic bug are easy to tell apart.

use city_avatar::{Avatar, AvatarConfig};
use city_camera::{Camera, CameraConfig, DISTANCES};
use city_hud::{self, HudFrame, HudInput};
use city_input::{action_for_key, InputAction, InputState};
use city_layout::{City, CityParams};
use city_math::Vec2;
use city_sky::{Sky, SkyClock, SkySample};

/// Fixed simulation step (s) — the world is stepped at a constant rate.
pub const STEP: f32 = 1.0 / 60.0;

/// Tuning of a session.
#[derive(Clone, Debug)]
pub struct WorldConfig {
    /// City generation parameters.
    pub params: CityParams,
    /// Simulated hours per real second.
    pub time_scale: f32,
    /// Starting time of day (hours).
    pub start_hours: f32,
    /// Rotate the radar with the view (`false`) or keep north up.
    pub north_up_radar: bool,
    /// Sun-arc azimuth.
    pub azimuth: f32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        WorldConfig {
            params: CityParams::default(),
            time_scale: 24.0 / 180.0, // one simulated day every 3 minutes
            start_hours: 8.0,
            north_up_radar: false,
            azimuth: city_sky::DEFAULT_AZIMUTH,
        }
    }
}

/// The running world.
pub struct World {
    city: City,
    sky: Sky,
    clock: SkyClock,
    avatar: Avatar,
    camera: Camera,
    input: InputState,
    hud_range: f32,
    north_up: bool,
    hud_visible: bool,
    dpr: f32,
    elapsed: f32,
    accumulator: f32,
    frames: u64,
    cfg: WorldConfig,
}

/// Snapshot for tests / diagnostics (JSON friendly).
#[derive(Clone, Debug, PartialEq)]
pub struct SimSnapshot {
    /// Simulated wall clock, `HH:MM`.
    pub clock: String,
    /// Hours `0..24`.
    pub hours: f32,
    /// Sky phase.
    pub phase: String,
    /// Sun elevation (degrees).
    pub sun_elev: f32,
    /// HDR exposure of the current instant.
    pub exposure: f32,
    /// Artificial light strength (windows/lamps).
    pub lamp: f32,
    pub player_x: f32,
    pub player_z: f32,
    pub speed_kmh: f32,
    pub sprinting: bool,
    pub grounded: bool,
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    pub cam_dist: f32,
    pub cam_index: usize,
    pub cam_count: usize,
    pub occluded: bool,
    pub walked: f32,
    pub buildings: usize,
    pub props: usize,
    pub roads: usize,
    pub frames: u64,
    pub hud_visible: bool,
    pub locked: bool,
    pub skipping: bool,
    pub tip: String,
}

impl World {
    /// Generate the city and place the character on its spawn point.
    pub fn new(cfg: WorldConfig) -> World {
        let city = City::generate(cfg.params.clone());
        let avatar = Avatar::spawn(&city, AvatarConfig::default());
        let mut camera = Camera::new(CameraConfig::default());
        camera.set_yaw(0.9);
        camera.snap(avatar.focus());
        World {
            city,
            sky: Sky::new(cfg.azimuth),
            clock: SkyClock::new(cfg.start_hours, cfg.time_scale),
            avatar,
            camera,
            input: InputState::new(),
            hud_range: city_hud::RADAR_RANGE,
            north_up: cfg.north_up_radar,
            hud_visible: true,
            dpr: 1.0,
            elapsed: 0.0,
            accumulator: 0.0,
            frames: 0,
            cfg,
        }
    }

    // --- accessors ------------------------------------------------------

    pub fn city(&self) -> &City {
        &self.city
    }
    pub fn avatar(&self) -> &Avatar {
        &self.avatar
    }
    pub fn camera(&self) -> &Camera {
        &self.camera
    }
    pub fn clock(&self) -> &SkyClock {
        &self.clock
    }
    pub fn input(&self) -> &InputState {
        &self.input
    }
    pub fn sky(&self) -> &Sky {
        &self.sky
    }
    /// Simulated time of day in hours.
    pub fn hours(&self) -> f32 {
        self.clock.hours()
    }
    /// Current sky sample.
    pub fn sample(&self) -> SkySample {
        self.sky.sample(self.clock.hours())
    }
    /// Real seconds elapsed since boot.
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }
    pub fn frames(&self) -> u64 {
        self.frames
    }
    /// The configuration this world was generated from.
    pub fn config(&self) -> &WorldConfig {
        &self.cfg
    }
    pub fn hud_visible(&self) -> bool {
        self.hud_visible
    }
    /// Simulated minutes still to the next phase (HUD).
    pub fn minutes_to_next_phase(&self) -> f32 {
        (self.clock.next_phase() - self.clock.hours()).max(0.0) * 60.0
    }
    pub fn set_hud_visible(&mut self, v: bool) {
        self.hud_visible = v;
    }
    /// Device pixel ratio of the canvas (set by the DOM layer).
    pub fn dpr(&self) -> f32 {
        self.dpr
    }
    /// Canvas scale factor (device pixels per CSS pixel).
    pub fn set_dpr(&mut self, dpr: f32) {
        self.dpr = if dpr.is_finite() { dpr } else { 1.0 };
    }
    /// Skip to the next phase of the day.
    pub fn time_skip(&mut self) {
        self.clock.skip_to_next_phase();
    }
    /// Set the simulated time of day (hours).
    pub fn set_hours(&mut self, hours: f32) {
        self.clock.set_hours(hours);
    }
    /// Deterministic camera test hook: jump to boom preset `index` and settle the rig.
    pub fn set_camera_index(&mut self, index: usize) {
        self.camera.set_distance_index(index);
        self.camera.update(&self.city, self.avatar.focus(), 0.0);
    }
    /// Move the character (used by tests and `respawn`).
    pub fn teleport(&mut self, at: Vec2) {
        self.avatar.teleport(&self.city, at);
        self.camera.snap(self.avatar.focus());
    }
    /// Frames drawn so far.
    pub fn set_frames(&mut self, n: u64) {
        self.frames = n as u64;
    }
    /// Mutable input state (the DOM layer needs this for focus resets).
    pub fn input_mut(&mut self) -> &mut InputState {
        &mut self.input
    }
    /// JSON snapshot for the browser diagnostics (hand-rolled; no serde).
    pub fn snapshot_json(&self) -> String {
        let s = self.snapshot();
        let mut o = String::with_capacity(512);
        use std::fmt::Write as _;
        let _ = writeln!(
            o,
            "{{\"clock\":\"{}\",\"hours\":{:.3},\"phase\":\"{}\",\"sun_elev\":{:.2},",
            s.clock, s.hours, s.phase, s.sun_elev
        );
        let _ = writeln!(
            o,
            "\"exposure\":{:.3},\"lamp\":{:.2},\"player_x\":{:.2},\"player_z\":{:.2},",
            s.exposure, s.lamp, s.player_x, s.player_z
        );
        let _ = writeln!(
            o,
            "\"speed_kmh\":{:.1},\"sprinting\":{},\"grounded\":{},\"cam_yaw\":{:.3},",
            s.speed_kmh, s.sprinting, s.grounded, s.cam_yaw
        );
        let _ = writeln!(
            o,
            "\"cam_pitch\":{:.2},\"cam_dist\":{:.1},\"cam_index\":{},\"cam_count\":{},",
            s.cam_pitch, s.cam_dist, s.cam_index, s.cam_count
        );
        let _ = writeln!(
            o,
            "\"occluded\":{},\"walked\":{:.1},\"buildings\":{},\"props\":{},\"roads\":{},",
            s.occluded, s.walked, s.buildings, s.props, s.roads
        );
        let _ = write!(
            o,
            "\"frames\":{},\"hud_visible\":{},\"locked\":{},\"skipping\":{},\"tip\":\"{}\"}}",
            s.frames,
            s.hud_visible,
            s.locked,
            s.skipping,
            s.tip.replace('\"', "")
        );
        o
    }

    // --- input ----------------------------------------------------------

    /// Feed a key event (`down = true` for keydown).
    pub fn key(&mut self, key: &str, down: bool) {
        if let Some(action) = action_for_key(key) {
            if down {
                self.input.press(action);
            } else {
                self.input.release(action);
            }
        }
    }

    /// Accumulate mouse movement (pixels).
    pub fn mouse(&mut self, dx: f32, dy: f32) {
        self.input.add_look(dx, dy);
    }

    /// Wheel zoom.
    pub fn wheel(&mut self, ticks: f32) {
        self.input.add_wheel(ticks);
    }

    /// Pointer-lock state changed.
    pub fn set_pointer_locked(&mut self, locked: bool) {
        self.input.pointer_locked = locked;
        if !locked {
            self.input.release_all();
        }
    }

    // --- stepping -------------------------------------------------------

    /// Jump the camera to a boom preset (index wraps around [`city_camera::DISTANCES`]).
    /// Advance the world by `dt` real seconds (fixed step internally).
    pub fn tick(&mut self, dt: f32) {
        let dt = if dt.is_finite() { dt.clamp(0.0, 0.25) } else { 0.0 };
        self.elapsed += dt;
        self.accumulator += dt;
        let mut guard = 0;
        while self.accumulator >= STEP && guard < 8 {
            self.step(STEP);
            self.accumulator -= STEP;
            guard += 1;
        }
        self.frames += 1;
    }

    /// One fixed simulation step.
    pub fn step(&mut self, dt: f32) {
        // hotkeys with edges must be handled before the input frame ends
        if self.input.just_pressed(InputAction::CycleCamera) {
            self.camera.cycle_distance();
        }
        if self.input.just_pressed(InputAction::TimeSkip) {
            self.clock.skip_to_next_phase();
        }
        if self.input.just_pressed(InputAction::ToggleHud) {
            self.hud_visible = !self.hud_visible;
        }
        if self.input.just_pressed(InputAction::Jump) {
            self.avatar.try_jump();
        }
        let wheel = self.input.take_wheel();
        if wheel.abs() > 1e-4 {
            self.camera.zoom(wheel);
        }

        // look: the camera owns the mouse delta
        let (dx, dy) = self.input.take_look();
        if self.input.pointer_locked {
            self.camera.look(dx, dy);
        }

        self.clock.advance(dt);

        let wish = self.input.move_axis();
        let sprint = self.input.held(InputAction::Sprint);
        self.avatar.update(&self.city, wish, self.camera.yaw(), sprint, dt);
        self.camera.update(&self.city, self.avatar.focus(), dt);
        self.input.end_frame();
    }

    // --- outputs --------------------------------------------------------

    /// HUD data for this frame (empty when hidden).
    pub fn hud_frame(&self) -> HudFrame {
        if !self.hud_visible {
            let mut f = HudFrame::default();
            f.clock = self.clock.clock();
            return f;
        }
        let input = HudInput {
            city: &self.city,
            pos: self.avatar.xz(),
            yaw: self.camera.yaw(),
            speed: self.avatar.speed(),
            sprinting: self.avatar.is_sprinting(),
            walked: self.avatar.distance_walked(),
            cam_index: self.camera.distance_index(),
            cam_count: DISTANCES.len(),
            locked: self.input.pointer_locked,
            skipping: self.clock.is_skipping(),
            clock: self.clock.clock(),
            phase: self.sample().phase().to_string(),
            range: self.hud_range,
            north_up: self.north_up,
            uptime: self.elapsed,
            tip: city_hud::context_tip(&self.city, self.avatar.xz(), &self.sample()),
        };
        city_hud::build(&input)
    }

    /// Diagnostics for the DOM / tests.
    pub fn snapshot(&self) -> SimSnapshot {
        let s = self.sample();
        SimSnapshot {
            clock: self.clock.clock(),
            hours: self.clock.hours(),
            phase: s.phase().to_string(),
            sun_elev: s.sun_elev_deg,
            exposure: s.exposure,
            lamp: s.lamp_light,
            player_x: round1(self.avatar.xz().x),
            player_z: round1(self.avatar.xz().y),
            speed_kmh: city_hud::round1(city_hud::speed_kmh(self.avatar.speed())),
            sprinting: self.avatar.is_sprinting(),
            grounded: self.avatar.is_grounded(),
            cam_yaw: round3(self.camera.yaw()),
            cam_pitch: round2(self.camera.pitch()),
            cam_dist: round2(self.camera.distance()),
            cam_index: self.camera.distance_index(),
            cam_count: DISTANCES.len(),
            occluded: self.camera.occluded(),
            walked: round1(self.avatar.distance_walked()),
            buildings: self.city.buildings().len(),
            props: self.city.props().len(),
            roads: self.city.roads().len(),
            frames: self.frames,
            hud_visible: self.hud_visible,
            locked: self.input.pointer_locked,
            skipping: self.clock.is_skipping(),
            tip: city_hud::context_tip(&self.city, self.avatar.xz(), &s),
        }
    }

    /// Spawn point of the city (used by tests that re-centre the view).
    pub fn spawn(&self) -> Vec2 {
        self.city.spawn_point()
    }
}

#[inline]
fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

#[inline]
fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

#[inline]
fn round3(v: f32) -> f32 {
    (v * 1000.0).round() / 1000.0
}
