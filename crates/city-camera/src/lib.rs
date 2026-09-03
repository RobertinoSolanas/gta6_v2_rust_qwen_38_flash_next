//! # city-camera
//!
//! Bounded context for *where the eye is*. A third-person orbit rig around a focus
//! point: mouse yaw/pitch with a clamped pitch, preset distances, exponential smoothing
//! and an occlusion pull-back so a building never swallows the character.
//!
//! Design notes:
//! * Yaw is the world yaw of the **camera → focus** direction: the eye sits behind the
//!   focus along `-forward`, and `forward = Vec2::from_angle(yaw)` is what the avatar
//!   uses for "W". Both live in one place on purpose — mirrored controls are a classic.
//! * Occlusion is a coarse ray march against the city's spatial index. The boom only
//!   shortens quickly and lengthens slowly, so the camera never pops.
//! * `view()` returns a right-handed matrix suitable for `uniformMatrix4fv`.

#![forbid(unsafe_code)]

use city_layout::City;
use city_math::{clamp, wrap_angle, Mat4, Vec2, Vec3};

/// Camera boom presets, metres behind the focus point.
pub const DISTANCES: [f32; 4] = [3.2, 5.0, 8.0, 11.0];

/// Vertical field of view (radians).
pub const FOV_Y: f32 = 1.08;

/// Near/far clipping planes (metres).
pub const NEAR: f32 = 0.12;
pub const FAR: f32 = 900.0;

/// Tuning of the rig.
#[derive(Clone, Copy, Debug)]
pub struct CameraConfig {
    /// Radians of yaw/pitch per pixel of mouse movement.
    pub sensitivity: f32,
    /// Lowest / lowest allowed pitch (radians).
    pub pitch_min: f32,
    pub pitch_max: f32,
    /// Focus follow rate (1/s).
    pub follow_rate: f32,
    /// How fast an occluded camera pulls in (1/s).
    pub pull_in_rate: f32,
    /// How fast a freed camera drifts back out (1/s).
    pub pull_out_rate: f32,
    /// Extra eye height above the ideal orbit height (m).
    pub height_offset: f32,
    /// Shortest legal boom after a pull-back (m).
    pub min_distance: f32,
    /// Probe radius for the occlusion march (m).
    pub probe_radius: f32,
    /// Number of ray-march samples.
    pub probe_steps: usize,
}

impl Default for CameraConfig {
    fn default() -> Self {
        CameraConfig {
            sensitivity: 0.0028,
            pitch_min: -1.05,
            pitch_max: 1.25,
            follow_rate: 26.0,
            pull_in_rate: 14.0,
            pull_out_rate: 2.4,
            height_offset: 0.12,
            min_distance: 1.2,
            probe_radius: 0.35,
            probe_steps: 12,
        }
    }
}

impl CameraConfig {
    /// Clamp a pitch into the legal range.
    #[inline]
    pub fn clamp_pitch(&self, pitch: f32) -> f32 {
        clamp(pitch, self.pitch_min, self.pitch_max)
    }
}

/// The orbit rig.
#[derive(Clone, Debug)]
pub struct Camera {
    yaw: f32,
    pitch: f32,
    distance: f32,
    eye: Vec3,
    focus: Vec3,
    boom: f32,
    distance_index: usize,
    cfg: CameraConfig,
}

impl Camera {
    /// Rig looking along `+X`, slightly above the horizon.
    pub fn new(cfg: CameraConfig) -> Camera {
        let mut cam = Camera {
            yaw: 0.0,
            pitch: 0.12,
            distance: DISTANCES[1],
            eye: Vec3::ZERO,
            focus: Vec3::ZERO,
            boom: DISTANCES[1],
            distance_index: 1,
            cfg,
        };
        cam.snap(Vec3::ZERO);
        cam
    }

    // --- state ----------------------------------------------------------

    /// World yaw of the camera→focus direction (radians) — the avatar's "forward".
    #[inline]
    pub fn yaw(&self) -> f32 {
        self.yaw
    }
    /// Pitch (radians, `+` = looking down).
    #[inline]
    pub fn pitch(&self) -> f32 {
        self.pitch
    }
    /// Requested boom length (m).
    #[inline]
    pub fn distance(&self) -> f32 {
        self.distance
    }
    /// Actual eye position (smoothed + occlusion corrected).
    #[inline]
    pub fn eye(&self) -> Vec3 {
        self.eye
    }
    /// Point the camera looks at.
    #[inline]
    pub fn focus(&self) -> Vec3 {
        self.focus
    }
    /// Active boom length after occlusion pull-back.
    #[inline]
    pub fn boom(&self) -> f32 {
        self.boom
    }
    /// Index of the active preset in [`DISTANCES`].
    #[inline]
    pub fn distance_index(&self) -> usize {
        self.distance_index
    }
    /// `true` while something blocks the view.
    #[inline]
    pub fn occluded(&self) -> bool {
        self.boom + 0.05 < self.distance
    }
    #[inline]
    pub fn config(&self) -> CameraConfig {
        self.cfg
    }

    /// Set yaw directly (spawn/teleport).
    pub fn set_yaw(&mut self, yaw: f32) {
        self.yaw = wrap_angle(yaw);
    }

    /// Set pitch directly (clamped).
    pub fn set_pitch(&mut self, pitch: f32) {
        self.pitch = self.cfg.clamp_pitch(pitch);
    }

    /// Apply a raw mouse delta (pixels).
    pub fn look(&mut self, dx: f32, dy: f32) {
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        self.yaw = wrap_angle(self.yaw - dx * self.cfg.sensitivity);
        self.pitch = self.cfg.clamp_pitch(self.pitch - dy * self.cfg.sensitivity);
    }

    /// Pick a boom preset by index (wraps around).
    pub fn set_distance_index(&mut self, index: usize) {
        self.distance_index = index % DISTANCES.len();
        self.distance = DISTANCES[self.distance_index];
    }

    /// `F`: next boom preset.
    pub fn cycle_distance(&mut self) {
        self.set_distance_index(self.distance_index + 1);
    }

    /// Wheel zoom (`ticks > 0` zooms in), snapped to the nearest preset.
    pub fn zoom(&mut self, ticks: f32) {
        let want = clamp(
            self.distance - ticks * 0.6,
            DISTANCES[0],
            DISTANCES[DISTANCES.len() - 1],
        );
        let mut best = self.distance_index;
        for (i, d) in DISTANCES.iter().enumerate() {
            if (d - want).abs() < (DISTANCES[best] - want).abs() {
                best = i;
            }
        }
        self.set_distance_index(best);
    }

    /// Snap the rig onto a focus point with no smoothing.
    pub fn snap(&mut self, focus: Vec3) {
        self.focus = focus;
        self.boom = self.distance;
        self.eye = self.eye_for(self.focus, self.boom);
    }

    /// Number of candidate lookups per axis (for debug/HUD).
    #[allow(dead_code)]
    fn probe_budget(&self) -> usize {
        self.cfg.probe_steps
    }

    /// Smoothly follow `focus`, resolving occlusion against `city`.
    pub fn update(&mut self, city: &City, focus: Vec3, dt: f32) {
        let dt = clamp(dt, 0.0, 0.1);
        let r = self.cfg.follow_rate;
        self.focus = Vec3::new(
            city_math::damp(self.focus.x, focus.x, r, dt),
            city_math::damp(self.focus.y, focus.y, r * 0.6, dt),
            city_math::damp(self.focus.z, focus.z, r, dt),
        );

        let target = self
            .blocked_at(city, self.focus, self.distance)
            .unwrap_or(self.distance)
            .max(self.cfg.min_distance);
        let rate = if target < self.boom {
            self.cfg.pull_in_rate
        } else {
            self.cfg.pull_out_rate
        };
        self.boom = city_math::damp(self.boom, target, rate, dt).min(self.distance);
        self.eye = self.eye_for(self.focus, self.boom);
    }

    /// Same as [`Camera::update`] without an occlusion world (flat test world).
    pub fn update_free(&mut self, focus: Vec3, dt: f32) {
        let dt = clamp(dt, 0.0, 0.1);
        let r = self.cfg.follow_rate;
        self.focus = Vec3::new(
            city_math::damp(self.focus.x, focus.x, r, dt),
            city_math::damp(self.focus.y, focus.y, r * 0.6, dt),
            city_math::damp(self.focus.z, focus.z, r, dt),
        );
        self.boom = city_math::damp(self.boom, self.distance, self.cfg.pull_out_rate, dt);
        self.eye = self.eye_for(self.focus, self.boom);
    }

    /// Unit view direction (eye → focus).
    pub fn view_dir(&self) -> Vec3 {
        let d = self.focus - self.eye;
        if d.len_sq() < 1e-12 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            d.norm()
        }
    }

    /// View matrix (right-handed, `-Z` forward).
    pub fn view(&self) -> Mat4 {
        Mat4::look_at(self.eye, self.focus, Vec3::new(0.0, 1.0, 0.0))
    }

    /// Projection matrix.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        Mat4::perspective(FOV_Y, aspect.max(0.01), NEAR, FAR)
    }

    /// Ideal eye position for a focus point and boom length.
    pub fn eye_for(&self, focus: Vec3, boom: f32) -> Vec3 {
        let flat = city_math::Vec2::from_angle(self.yaw);
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let forward = Vec3::new(flat.x * cp, -sp, flat.y * cp);
        Vec3::new(
            focus.x - forward.x * boom,
            focus.y - forward.y * boom + self.cfg.height_offset,
            focus.z - forward.z * boom,
        )
    }

    /// Distance from `focus` at which the boom would hit a building, if at all.
    fn blocked_at(&self, city: &City, focus: Vec3, boom: f32) -> Option<f32> {
        let dir = self.eye_for(focus, boom) - focus;
        if dir.len_sq() < 1e-12 {
            return None;
        }
        let dir = dir.norm();
        let steps = self.cfg.probe_steps.max(4);
        let step = boom / steps as f32;
        for i in 1..=steps {
            let p = focus + dir * (step * i as f32);
            if blocked(city, p, self.cfg.probe_radius) {
                return Some((i as f32 * step - step * 0.6).max(self.cfg.min_distance));
            }
        }
        None
    }
}

/// `true` when a *tall enough* solid occupies `p` (kerbs and bollards are ignored so the
/// camera is not pulled in by street furniture).
fn blocked(city: &City, p: Vec3, radius: f32) -> bool {
    let here = Vec2::new(p.x, p.z);
    for id in city.index().candidates(here, radius) {
        if let Some(item) = city.index().item(id) {
            if item.height > 1.0 && item.solid.grown(radius).contains(here) {
                return true;
            }
        }
    }
    false
}

/// Convenience wrapper: view matrix from eye/focus.
pub fn look_from(eye: Vec3, focus: Vec3) -> Mat4 {
    Mat4::look_at(eye, focus, Vec3::new(0.0, 1.0, 0.0))
}

/// Wrap a yaw delta into `(-PI, PI]`.
#[inline]
pub fn wrap_yaw(a: f32) -> f32 {
    wrap_angle(a)
}
