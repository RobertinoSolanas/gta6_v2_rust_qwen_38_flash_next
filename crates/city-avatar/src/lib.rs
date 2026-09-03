//! # city-avatar
//!
//! Bounded context for *the character the player drives*: camera-relative walking,
//! sprinting, jumping, gravity, ground follow and wall sliding, plus the joint angles of
//! a walk/sprint/idle pose that a renderer can build a mesh around.
//!
//! Design notes:
//! * [`City`] is treated as a pure *collision + terrain* collaborator: the avatar only
//!   ever calls `resolve` / `is_walkable` on it, never the other way round.
//! * Movement is velocity based (acceleration + drag), so the walk-cycle phase can be
//!   derived from the actual ground speed instead of "is a key down".
//! * The pose is pure data ([`AvatarPose`]) — bones are built from it at draw time. No
//!   skeleton asset exists anywhere in this project.
//! * `eye_height` is the only vertical reference: the feet are at `pos.y`, the camera
//!   aims at `pos.y + eye_height`.

#![forbid(unsafe_code)]

use city_layout::City;
use city_math::{clamp, wrap_angle, Vec2, Vec3, TAU};

/// Tuning of one character.
#[derive(Clone, Copy, Debug)]
pub struct AvatarConfig {
    /// Walk speed (m/s).
    pub walk_speed: f32,
    /// Sprint speed (m/s).
    pub sprint_speed: f32,
    /// How quickly the body reaches the wish velocity (1/s).
    pub accel: f32,
    /// Ground friction while nothing is pushed (1/s).
    pub drag: f32,
    /// Downward acceleration (m/s²).
    pub gravity: f32,
    /// Upward impulse when jumping (m/s).
    pub jump_speed: f32,
    /// Collision cylinder radius (m).
    pub radius: f32,
    /// Eye point above the feet (m).
    pub eye_height: f32,
    /// How fast the body turns towards the travel direction (1/s).
    pub turn_rate: f32,
    /// Kerb height the character simply walks up (m).
    pub step_height: f32,
    /// Walk-cycle stride length while walking (m per full cycle).
    pub stride_walk: f32,
    /// Sprint cycle length (m).
    pub stride_sprint: f32,
}

impl Default for AvatarConfig {
    fn default() -> Self {
        AvatarConfig {
            walk_speed: 2.6,
            sprint_speed: 5.6,
            accel: 16.0,
            drag: 16.0,
            gravity: 22.0,
            jump_speed: 5.4,
            radius: 0.42,
            eye_height: 1.66,
            turn_rate: 16.0,
            step_height: 0.45,
            stride_walk: 1.55,
            stride_sprint: 2.4,
        }
    }
}

/// Ground profile the avatar stands on. In the browser this is the sidewalk/park height
/// field of the generated city; tests supply their own.
pub trait Terrain {
    /// Surface height (m) under `p`.
    fn ground_y(&self, p: Vec2) -> f32;
    /// Slide a circle of `radius` at `p` out of solid geometry.
    fn resolve(&self, p: Vec2, radius: f32) -> Vec2;
}

impl Terrain for City {
    /// The generated city is flat (all kerbs and floors sit on `y = 0`).
    #[inline]
    fn ground_y(&self, _p: Vec2) -> f32 {
        0.0
    }
    #[inline]
    fn resolve(&self, p: Vec2, radius: f32) -> Vec2 {
        City::resolve(self, p, radius)
    }
}

/// Where the character is and how it moves.
#[derive(Clone, Debug)]
pub struct Avatar {
    /// Feet position (world).
    pos: Vec3,
    /// Horizontal velocity (m/s).
    vel: Vec2,
    /// Vertical velocity (m/s).
    vy: f32,
    /// Facing yaw (radians, `0` = +X).
    yaw: f32,
    /// Walk-cycle phase in turns (`0..1`).
    stride: f32,
    /// `true` while standing on something.
    grounded: bool,
    /// Cached ground speed (m/s).
    speed: f32,
    /// `true` while sprinting.
    sprinting: bool,
    /// Total distance walked (m) — HUD odometry.
    distance: f32,
    cfg: AvatarConfig,
}

/// Procedural body pose: joint angles in radians (see [`Avatar::pose`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvatarPose {
    /// Torso yaw offset relative to the facing direction.
    pub torso_twist: f32,
    /// Torso pitch (+ = leaning forwards).
    pub torso_pitch: f32,
    /// Left / right upper-arm swing around the shoulder.
    pub arm_l: f32,
    pub arm_r: f32,
    /// Left / right thigh swing around the hip.
    pub leg_l: f32,
    pub leg_r: f32,
    /// Head pitch (+ = looking up).
    pub head_pitch: f32,
    /// Vertical bob of the whole body (m, always <= 0).
    pub bob: f32,
    /// `true` while airborne.
    pub airborne: bool,
}

impl Default for AvatarPose {
    fn default() -> Self {
        AvatarPose {
            torso_twist: 0.0,
            torso_pitch: 0.0,
            arm_l: 0.0,
            arm_r: 0.0,
            leg_l: 0.0,
            leg_r: 0.0,
            head_pitch: 0.0,
            bob: 0.0,
            airborne: false,
        }
    }
}

impl Avatar {
    /// Spawn on the city's guaranteed walkable spawn point.
    pub fn spawn(city: &City, cfg: AvatarConfig) -> Avatar {
        let feet = city.spawn_point();
        Avatar::at(city, feet, cfg)
    }

    /// Spawn at an explicit XZ position on flat ground.
    pub fn at(city: &City, feet: Vec2, cfg: AvatarConfig) -> Avatar {
        let fixed = city.resolve(feet, cfg.radius);
        Avatar {
            pos: Vec3::new(fixed.x, 0.0, fixed.y),
            vel: Vec2::ZERO,
            vy: 0.0,
            yaw: 0.0,
            stride: 0.0,
            grounded: true,
            speed: 0.0,
            sprinting: false,
            distance: 0.0,
            cfg,
        }
    }

    // --- state ----------------------------------------------------------

    #[inline]
    pub fn config(&self) -> AvatarConfig {
        self.cfg
    }
    /// Feet position.
    #[inline]
    pub fn position(&self) -> Vec3 {
        self.pos
    }
    /// Horizontal position.
    #[inline]
    pub fn xz(&self) -> Vec2 {
        self.pos.xz()
    }
    /// Eye point (what the camera looks over).
    #[inline]
    pub fn eye(&self) -> Vec3 {
        Vec3::new(self.pos.x, self.pos.y + self.cfg.eye_height, self.pos.z)
    }
    /// Point the camera orbits around.
    #[inline]
    pub fn focus(&self) -> Vec3 {
        Vec3::new(self.pos.x, self.pos.y + self.cfg.eye_height * 0.82, self.pos.z)
    }
    /// Facing yaw (radians).
    #[inline]
    pub fn yaw(&self) -> f32 {
        self.yaw
    }
    /// Ground speed (m/s).
    #[inline]
    pub fn speed(&self) -> f32 {
        self.speed
    }
    /// Horizontal velocity (m/s).
    #[inline]
    pub fn velocity(&self) -> Vec2 {
        self.vel
    }
    /// Vertical velocity (m/s).
    #[inline]
    pub fn vertical_speed(&self) -> f32 {
        self.vy
    }
    #[inline]
    pub fn is_grounded(&self) -> bool {
        self.grounded
    }
    #[inline]
    pub fn is_sprinting(&self) -> bool {
        self.sprinting
    }
    /// Walk-cycle phase in `0..1` (`0.5` = the other foot forward).
    #[inline]
    pub fn phase(&self) -> f32 {
        self.stride
    }
    /// Total distance walked (m).
    #[inline]
    pub fn distance_walked(&self) -> f32 {
        self.distance
    }

    /// Aim the body immediately (spawn / teleport).
    pub fn face(&mut self, yaw: f32) {
        self.yaw = wrap_angle(yaw);
    }

    // --- simulation -----------------------------------------------------

    /// One simulation step.
    ///
    /// * `wish` — camera-relative wish direction `(right, forward)`, length `0..=1`.
    /// * `camera_yaw` — camera yaw, so "forward" means *away from the camera*.
    /// * `sprint` — sprint key held.
    pub fn update(&mut self, city: &City, wish: Vec2, camera_yaw: f32, sprint: bool, dt: f32) {
        self.step(city, wish, camera_yaw, sprint, dt);
    }

    /// One simulation step against an arbitrary terrain (used by tests).
    #[allow(dead_code)]
    fn update_unused(&mut self, city: &City, wish: Vec2, camera_yaw: f32, sprint: bool, dt: f32) {
        self.update(city, wish, camera_yaw, sprint, dt);
    }

    /// One simulation step against an arbitrary terrain (used by tests).
    pub fn update_on(
        &mut self,
        terrain: &dyn Terrain,
        wish: Vec2,
        camera_yaw: f32,
        sprint: bool,
        dt: f32,
    ) {
        self.step_impl(
            &|p: Vec2, r: f32| terrain.resolve(p, r),
            &|p: Vec2| terrain.ground_y(p),
            wish,
            camera_yaw,
            sprint,
            dt,
        );
    }

    fn step(&mut self, city: &City, wish: Vec2, camera_yaw: f32, sprint: bool, dt: f32) {
        self.step_impl(
            &|p, r| City::resolve(city, p, r),
            &|_| 0.0,
            wish,
            camera_yaw,
            sprint,
            dt,
        );
    }

    fn step_impl(
        &mut self,
        resolve: &dyn Fn(Vec2, f32) -> Vec2,
        ground: &dyn Fn(Vec2) -> f32,
        wish: Vec2,
        camera_yaw: f32,
        sprint: bool,
        dt: f32,
    ) {
        let dt = clamp(dt, 0.0, 0.1);
        if dt <= 0.0 {
            return;
        }

        // 1. wish direction in world space (camera-relative)
        let fwd = Vec2::from_angle(camera_yaw);
        let right = fwd.perp();
        let w = if wish.len_sq() > 1.0 {
            wish.clamp_len(1.0)
        } else {
            wish
        };
        let dir = right * w.x + fwd * w.y;
        let pushing = dir.len_sq() > 1e-6;
        self.sprinting = sprint && pushing;

        // 2. accelerate towards the wish velocity, drag when idle
        let want_speed = if self.sprinting {
            self.cfg.sprint_speed
        } else {
            self.cfg.walk_speed
        };
        let target = dir * want_speed;
        if pushing {
            let dv = target - self.vel;
            self.vel = self.vel + dv.clamp_len(self.cfg.accel * dt);
            self.vel = self.vel.clamp_len(want_speed);
        } else {
            let drop = self.cfg.drag * dt * self.vel.len();
            self.vel = if self.vel.len() <= drop {
                Vec2::ZERO
            } else {
                self.vel - self.vel.norm() * drop
            };
        }

        // 3. integrate horizontally, slide along walls
        let attempted = self.pos.xz() + self.vel * dt;
        let fixed = resolve(attempted, self.cfg.radius);
        if fixed.dist_sq(attempted) > 1e-10 {
            let n = (fixed - attempted).norm();
            let into = self.vel.dot(n);
            if into < 0.0 {
                self.vel = self.vel - n * into;
            }
        }
        let moved = fixed;
        self.distance += self.vel.len() * dt;

        // 4. vertical: gravity + ground follow (kerbs become walk-up steps)
        self.vy -= self.cfg.gravity * dt;
        let ground_y = ground(moved);
        let rise = ground_y - self.pos.y;
        let mut y = self.pos.y + self.vy * dt;
        if self.grounded && rise > 0.0 && rise <= self.cfg.step_height {
            y = ground_y;
            self.vy = 0.0;
        }
        if y <= ground_y {
            y = ground_y;
            self.vy = 0.0;
            self.grounded = true;
        } else if y - ground_y > 0.02 {
            self.grounded = false;
        }
        self.pos = Vec3::new(moved.x, y, moved.y);

        // 5. face the direction of travel
        self.speed = self.vel.len();
        if self.speed > 0.2 {
            let want = self.vel.angle();
            self.yaw = wrap_angle(self.yaw + wrap_angle(want - self.yaw) * turn_gain(self.cfg.turn_rate, dt));
        }

        // 6. advance the walk cycle from actual speed
        let stride_len = if self.sprinting {
            self.cfg.stride_sprint
        } else {
            self.cfg.stride_walk
        };
        if self.grounded && self.speed > 0.15 {
            self.stride = wrap01(self.stride + self.speed * dt / stride_len);
        } else if !self.grounded {
            // airborne: hold the pose
        } else {
            self.stride = move_towards(self.stride, 0.0, dt);
        }
    }

    /// Jump while on the ground. Returns `true` when the jump was taken.
    pub fn try_jump(&mut self) -> bool {
        if !self.grounded {
            return false;
        }
        self.vy = self.cfg.jump_speed;
        self.grounded = false;
        true
    }

    /// Move the character somewhere else and settle it on the ground.
    pub fn teleport(&mut self, city: &City, at: Vec2) {
        let fixed = city.resolve(at, self.cfg.radius);
        self.pos = Vec3::new(fixed.x, 0.0, fixed.y);
        self.vel = Vec2::ZERO;
        self.vy = 0.0;
        self.speed = 0.0;
        self.grounded = true;
    }

    /// Body pose for the current motion state.
    ///
    /// `camera_pitch` is the camera pitch (radians, `+` = looking up at the sky); the
    /// head counter-rotates a little so the character does not stare at the ground.
    pub fn pose(&self, camera_pitch: f32) -> AvatarPose {
        let head = clamp(-camera_pitch, -0.6, 0.6);
        if !self.grounded {
            let t = clamp(self.vy / self.cfg.jump_speed, -1.0, 1.0);
            return AvatarPose {
                torso_twist: 0.06,
                torso_pitch: -0.20 + 0.12 * t,
                arm_l: -1.9 + 0.4 * t,
                arm_r: -1.6 + 0.4 * t,
                leg_l: -0.6 - 0.25 * t,
                leg_r: 0.45 + 0.2 * t,
                head_pitch: head * 0.3,
                bob: 0.04,
                airborne: true,
            };
        }
        if self.speed <= 0.25 {
            // idle: shallow breathing, arms hanging
            let breathe = (self.stride * TAU).sin() * 0.02;
            return AvatarPose {
                arm_l: breathe - 0.04,
                arm_r: -breathe + 0.04,
                bob: -breathe.abs() * 0.4,
                head_pitch: head * 0.35,
                ..AvatarPose::default()
            };
        }
        let amp = if self.sprinting { 1.0 } else { 0.7 };
        let w = self.stride * TAU;
        let swing = w.sin();
        AvatarPose {
            torso_twist: -swing * 0.10,
            torso_pitch: 0.10 * amp + 0.10 * clamp(self.speed / self.cfg.sprint_speed, 0.0, 1.0),
            arm_l: -swing * 0.85 * amp,
            arm_r: swing * 0.85 * amp,
            leg_l: (w + core::f32::consts::PI).sin() * 0.55 * amp,
            leg_r: swing * 0.55 * amp,
            head_pitch: head * 0.25,
            bob: -((2.0 * w + 0.5).sin().abs()) * 0.035 * amp,
            airborne: false,
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Frame-rate independent turn factor (0..=1).
#[inline]
fn turn_gain(rate: f32, dt: f32) -> f32 {
    city_math::damp(0.0, 1.0, rate, dt).min(1.0)
}

#[inline]
fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    city_math::move_towards(current, target, max_delta)
}

/// Wrap a phase into `0..1`.
#[inline]
pub fn wrap01(x: f32) -> f32 {
    let m = x % 1.0;
    if m < 0.0 {
        m + 1.0
    } else {
        m
    }
}
