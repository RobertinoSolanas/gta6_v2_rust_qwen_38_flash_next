//! # city-sky
//!
//! Bounded context for the day/night cycle. It owns *when the light is*, and nothing
//! else: sun & moon direction, a cheap two-lobe sky gradient, fog colour and density,
//! HDR exposure, the star field fade, and the intensity curves that other contexts use
//! for window lights, street lamps, headlights and the ambient floor.
//!
//! Design notes:
//! * Time of day is a scalar `hours` in `0..24`. [`Sky::sample`] is stateless, so any
//!   instant can be evaluated (and unit tested) directly; [`SkyClock`] is the only
//!   stateful piece (advance, skip, wrap).
//! * The sun travels a tilted circular arc whose horizontal direction is the city
//!   `azimuth`, so sunrise does not have to point down a specific street.
//! * Every colour band and scalar ramp is a closed-form curve built from
//!   `smoothstep`/`smootherstep` (`edge0`, `edge1`, `x` argument order!) — no tables to
//!   keep in sync, every ramp is monotone and reproducible. Pass `edge0 > edge1` for a
//!   curve that falls as `x` grows.
//!
//! Reference values (default azimuth): sunrise ≈ 06:00, noon elevation ≈ +57°, sunset
//! 18:00, sun at -12° around 18:38 and 05:22 — that twilight band is where lamps,
//! windows, fog and exposure all move.

#![forbid(unsafe_code)]

use city_math::{smootherstep, smoothstep, PI};

/// Length of a full day in hours.
pub const DAY_LENGTH: f32 = 24.0;

/// Default simulated hours advanced per real second (a 4 minute day by default:
/// `24 h / 240 s`, i.e. one simulated minute per second).
pub const DEFAULT_TIME_SCALE: f32 = 24.0 / 240.0;

/// How many real seconds one `SkyClock::skip_to_next_phase()` takes.
pub const SKIP_SECONDS: f32 = 1.5;

/// Default sun-arc azimuth.
pub const DEFAULT_AZIMUTH: f32 = 0.6;

/// Sun elevation (degrees) where "day" stops: the top of the twilight band.
pub const TWILIGHT_START: f32 = -2.0;

/// Sun elevation (negative) where twilight ends and night is complete.
pub const NIGHT_FULL: f32 = -12.0;

/// Wrap raw hours into `0..24`.
#[inline]
pub fn wrap_hours(h: f32) -> f32 {
    let m = h % DAY_LENGTH;
    if m < 0.0 {
        m + DAY_LENGTH
    } else {
        m
    }
}

/// Format hours as `HH:MM`.
pub fn format_clock(hours: f32) -> String {
    let h = wrap_hours(hours);
    let mut hh = h.floor() as i32;
    let mut mm = ((h - h.floor()) * 60.0).round() as i32;
    if mm == 60 {
        mm = 0;
        hh = (hh + 1) % 24;
    }
    format!("{hh:02}:{mm:02}")
}

// ---------------------------------------------------------------------------
// sun / moon geometry
// ---------------------------------------------------------------------------

/// Unit direction of the sun for `hours` (0..24), arc rising towards `azimuth`.
///
/// Elevation is `-90°` at midnight, `+90°` at the noon peak; the tip of the vector
/// traces a great circle.
pub fn sun_dir(hours: f32, azimuth: f32) -> city_math::Vec3 {
    let tilt = azimuth - PI / 2.0;
    let p = (hours / DAY_LENGTH - 0.25) * 2.0 * PI;
    let (sp, cp) = p.sin_cos();
    let (ct, st) = tilt.sin_cos();
    city_math::Vec3::new(cp * ct, sp, -cp * st)
}

/// The moon is exactly opposite the sun.
pub fn moon_dir(hours: f32, azimuth: f32) -> city_math::Vec3 {
    let s = sun_dir(hours, azimuth);
    city_math::Vec3::new(-s.x, -s.y, -s.z)
}

/// Sun elevation in degrees (`-90..90`).
pub fn sun_elevation_deg(hours: f32, azimuth: f32) -> f32 {
    sun_dir(hours, azimuth).y.to_degrees()
}

/// `true` while the sun is at or above the horizon.
pub fn is_daytime(hours: f32, azimuth: f32) -> bool {
    sun_dir(hours, azimuth).y >= 0.0
}

// ---------------------------------------------------------------------------
// scalar helpers
// ---------------------------------------------------------------------------

/// Ramp from `1` down to `0` between `from` and `to` (`from > to` reads naturally:
/// `ramp_down(12.0, 2.0, elevation)` is 1 below 2° and 0 above 12°).
#[inline]
fn ramp_down(from: f32, to: f32, x: f32) -> f32 {
    smoothstep(from, to, x)
}

/// Band window: `0` outside `[lo, hi]`, `1` in the middle third. Used for the sunset
/// glow, which must vanish both at noon and in deep night.
#[inline]
fn band(lo: f32, hi: f32, x: f32) -> f32 {
    smoothstep(lo, lo + (hi - lo) * 0.35, x) * smoothstep(hi, lo + (hi - lo) * 0.65, x)
}

// ---------------------------------------------------------------------------
// colour helpers
// ---------------------------------------------------------------------------

#[inline]
fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = city_math::saturate(t);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[inline]
fn scale3(c: [f32; 3], s: f32) -> [f32; 3] {
    [c[0] * s, c[1] * s, c[2] * s]
}

#[inline]
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// "Deep night" zenith colour.
const NIGHT: [f32; 3] = [0.015, 0.023, 0.045];
/// "Deep night" horizon colour.
const NIGHT_HORIZON: [f32; 3] = [0.035, 0.052, 0.095];
/// Sun elevation at which the sky has fully reached its day colours.
const SKY_FULL_DAY: f32 = 14.0;

/// Colour of the sky *straight overhead*.
pub fn zenith_color(elev_deg: f32) -> [f32; 3] {
    let t = smoothstep(NIGHT_FULL, SKY_FULL_DAY, elev_deg);
    let deep = mix3(
        [0.05, 0.11, 0.30],
        [0.20, 0.38, 0.78],
        smoothstep(NIGHT_FULL, 34.0, elev_deg),
    );
    mix3(NIGHT, deep, t)
}

/// Colour of the sky at the horizon (night grey-blue -> pale daytime haze).
pub fn horizon_color(elev_deg: f32) -> [f32; 3] {
    let t = smoothstep(NIGHT_FULL, SKY_FULL_DAY, elev_deg);
    let pale = mix3(
        NIGHT_HORIZON,
        [0.62, 0.74, 0.90],
        smoothstep(NIGHT_FULL, 34.0, elev_deg),
    );
    mix3(NIGHT_HORIZON, pale, t)
}

/// How strong the sunset/sunrise glow is: `0` above `+12°` and below `-12°`, `1` on the
/// horizon.
pub fn glow_strength(elev_deg: f32) -> f32 {
    band(-12.0, 12.0, elev_deg)
}

/// Warm band around the sun's azimuth at the horizon.
pub fn glow_color(elev_deg: f32) -> [f32; 3] {
    let t = glow_strength(elev_deg);
    scale3(
        mix3(
            [1.00, 0.42, 0.12],
            [1.00, 0.72, 0.38],
            smoothstep(-6.0, 8.0, elev_deg),
        ),
        t,
    )
}

/// Colour the sun takes (red near the horizon, white high up).
pub fn sun_tint(elev_deg: f32) -> [f32; 3] {
    mix3(
        [1.00, 0.55, 0.26],
        [1.00, 0.96, 0.88],
        smoothstep(0.0, 18.0, elev_deg),
    )
}

/// Directional light colour: warm sun above the horizon, cool moonlight below.
pub fn light_color(elev_deg: f32) -> [f32; 3] {
    if elev_deg >= 0.0 {
        scale3(sun_tint(elev_deg), smoothstep(-1.5, 4.5, elev_deg))
    } else {
        scale3([0.34, 0.44, 0.68], moonlight(elev_deg) * 0.55)
    }
}

/// Fog / atmosphere base colour (horizon sky plus a little glow).
pub fn fog_color(elev_deg: f32) -> [f32; 3] {
    add3(
        horizon_color(elev_deg),
        scale3(glow_color(elev_deg), glow_strength(elev_deg) * 0.35),
    )
}

// ---------------------------------------------------------------------------
// scalar curves
// ---------------------------------------------------------------------------

/// Normalised daylight: `0` below [`TWILIGHT_START`], `1` above +12°.
pub fn daylight(elev_deg: f32) -> f32 {
    smoothstep(TWILIGHT_START, 12.0, elev_deg)
}

/// How far into the night we are: `0` while the sun is above [`TWILIGHT_START`], `1`
/// once it is [`NIGHT_FULL`] degrees down. Shared driver of exposure and fog density so
/// they cannot drift apart.
pub fn nightness(elev_deg: f32) -> f32 {
    smootherstep(TWILIGHT_START, NIGHT_FULL, elev_deg)
}

/// Star field fade: `1` deep night, `0` above +6°.
pub fn star_fade(elev_deg: f32) -> f32 {
    smootherstep(6.0, -8.0, elev_deg)
}

/// Artificial window light: ramps up through dusk, `1` by ~18:25, `0` by mid-morning.
pub fn window_light(elev_deg: f32) -> f32 {
    ramp_down(4.0, -3.0, elev_deg)
}

/// Street lamps / neon: switch on a touch later than the windows.
pub fn lamp_light(elev_deg: f32) -> f32 {
    ramp_down(3.0, -3.0, elev_deg)
}

/// Car headlights.
pub fn headlight(elev_deg: f32) -> f32 {
    ramp_down(3.5, -2.5, elev_deg)
}

/// Moonlight amount (cool fill light); `1` once the sun is 20° down.
pub fn moonlight(elev_deg: f32) -> f32 {
    smoothstep(0.0, -20.0, elev_deg)
}

/// Ambient (hemisphere) floor so geometry never goes fully black.
pub fn ambient(elev_deg: f32) -> f32 {
    0.06 + 0.49 * daylight(elev_deg)
}

/// How far the fog reaches in metres: full range by day, hazy (shorter) at night and
/// around the horizon.
pub fn fog_view_distance(elev_deg: f32) -> f32 {
    let base = 260.0 + 360.0 * daylight(elev_deg);
    base * (1.0 - 0.35 * glow_strength(elev_deg))
}

/// HDR exposure: exactly `1.0` while there is daylight, up to `2.1` in "night mode"
/// (sun ≥ 12° below the horizon).
pub fn exposure(elev_deg: f32) -> f32 {
    1.0 + 1.1 * nightness(elev_deg)
}

// ---------------------------------------------------------------------------
// sampled snapshot
// ---------------------------------------------------------------------------

/// Everything the renderer / HUD needs for one instant, as plain data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkySample {
    /// Hours this sample was taken at (in `0..24` when sampled from [`SkyClock`]).
    pub hours: f32,
    /// Unit sun direction.
    pub sun: city_math::Vec3,
    /// Unit moon direction.
    pub moon: city_math::Vec3,
    /// Sun elevation in degrees.
    pub sun_elev_deg: f32,
    /// Sky straight up.
    pub zenith: [f32; 3],
    /// Sky at the horizon.
    pub horizon: [f32; 3],
    /// Sunset/sunrise glow colour.
    pub glow: [f32; 3],
    /// `0..1` glow strength.
    pub glow_strength: f32,
    /// Fog colour.
    pub fog: [f32; 3],
    /// Fog view distance in metres.
    pub fog_distance: f32,
    /// Directional (sun/moon) light colour, pre-scaled.
    pub light_color: [f32; 3],
    /// Directional light strength (`0` for the moon, `0.45..1` for the sun).
    pub light_strength: f32,
    /// `0..1` ambient floor.
    pub ambient: f32,
    /// `0..1` window emissive curve.
    pub window_light: f32,
    /// `0..1` street lamp / neon curve.
    pub lamp_light: f32,
    /// `0..1` headlight curve.
    pub headlight: f32,
    /// `0..1` star fade.
    pub star_fade: f32,
    /// `0..1` how "night" it is (exposure / post fallback).
    pub night: f32,
    /// HDR exposure multiplier.
    pub exposure: f32,
}

impl SkySample {
    /// `true` while the sun is up.
    #[inline]
    pub fn is_daytime(&self) -> bool {
        self.sun_elev_deg >= 0.0
    }

    /// `true` in the narrow sunrise/sunset band (|elevation| < 8°).
    #[inline]
    pub fn is_transition(&self) -> bool {
        self.sun_elev_deg.abs() < 8.0
    }

    /// Human readable phase name (ASCII, HUD friendly).
    pub fn phase(&self) -> &'static str {
        let e = self.sun_elev_deg;
        let rising = (self.hours % DAY_LENGTH) < 12.0;
        if e >= 15.0 {
            "day"
        } else if e >= 0.0 {
            if rising {
                "morning"
            } else {
                "evening"
            }
        } else if e > NIGHT_FULL {
            if rising {
                "dawn"
            } else {
                "dusk"
            }
        } else {
            "night"
        }
    }
}

/// Stateless sky model; `azimuth` rotates the sun arc inside the city.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    pub azimuth: f32,
}

impl Default for Sky {
    fn default() -> Self {
        Sky {
            azimuth: DEFAULT_AZIMUTH,
        }
    }
}

impl Sky {
    /// Build a sky whose sun arc rises towards horizontal direction `azimuth`.
    pub fn new(azimuth: f32) -> Self {
        Sky { azimuth }
    }

    /// Sample the whole model at `hours`.
    pub fn sample(&self, hours: f32) -> SkySample {
        // `hours` is *not* wrapped: the trig is periodic, so `sample(h) == sample(h+24)`
        // bit for bit, which keeps screenshot tests comparable.
        let sun = sun_dir(hours, self.azimuth);
        let moon = moon_dir(hours, self.azimuth);
        let e = sun.y.to_degrees();
        SkySample {
            hours,
            sun,
            moon,
            sun_elev_deg: e,
            zenith: zenith_color(e),
            horizon: horizon_color(e),
            glow: glow_color(e),
            glow_strength: glow_strength(e),
            fog: fog_color(e),
            fog_distance: fog_view_distance(e),
            light_color: light_color(e),
            light_strength: sun_light_strength(e),
            ambient: ambient(e),
            window_light: window_light(e),
            lamp_light: lamp_light(e),
            headlight: headlight(e),
            star_fade: star_fade(e),
            night: nightness(e),
            exposure: exposure(e),
        }
    }

    /// Sun light strength (the moon contributes through [`SkySample::light_color`]).
    pub fn light_strength(&self, hours: f32) -> f32 {
        sun_light_strength(sun_elevation_deg(hours, self.azimuth))
    }

    /// `HH:MM` string for `hours`.
    pub fn clock(&self, hours: f32) -> String {
        format_clock(hours)
    }
}

fn sun_light_strength(elev_deg: f32) -> f32 {
    if elev_deg <= 0.0 {
        0.0
    } else {
        0.45 + 0.55 * smoothstep(0.0, 25.0, elev_deg)
    }
}

// ---------------------------------------------------------------------------
// clock
// ---------------------------------------------------------------------------

/// Stateful time of day: advance with [`SkyClock::advance`], jump with
/// [`SkyClock::skip_to_next_phase`].
#[derive(Clone, Debug, PartialEq)]
pub struct SkyClock {
    hours: f32,
    scale: f32,
    skip_remaining: f32,
    skip_from: f32,
    skip_target: f32,
}

impl Default for SkyClock {
    fn default() -> Self {
        SkyClock::new(7.25, DEFAULT_TIME_SCALE)
    }
}

impl SkyClock {
    /// Start at `hours`, running `scale` simulated hours per real second.
    pub fn new(hours: f32, scale: f32) -> Self {
        SkyClock {
            hours: wrap_hours(hours),
            scale: if scale.is_finite() { scale } else { 0.0 },
            skip_remaining: 0.0,
            skip_from: 0.0,
            skip_target: 0.0,
        }
    }

    /// Current hours in `0..24`.
    #[inline]
    pub fn hours(&self) -> f32 {
        self.hours
    }

    /// Simulated hours per real second.
    #[inline]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// `HH:MM`.
    pub fn clock(&self) -> String {
        format_clock(self.hours)
    }

    /// `true` while a skip animation drives time.
    #[inline]
    pub fn is_skipping(&self) -> bool {
        self.skip_remaining > 0.0
    }

    /// Advance by `dt` real seconds; wraps at 24:00. While skipping, the skip animation
    /// drives time instead of `scale`.
    pub fn advance(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        if self.is_skipping() {
            self.skip_remaining -= dt;
            if self.skip_remaining <= 0.0 {
                self.hours = wrap_hours(self.skip_target);
                self.skip_remaining = 0.0;
            } else {
                let t = smootherstep(0.0, SKIP_SECONDS, SKIP_SECONDS - self.skip_remaining);
                let span = self.skip_target - self.skip_from;
                self.hours = wrap_hours(self.skip_from + span * t);
            }
            return;
        }
        self.hours = wrap_hours(self.hours + dt * self.scale);
    }

    /// Jump to an absolute time of day.
    pub fn set_hours(&mut self, hours: f32) {
        if hours.is_finite() {
            self.hours = wrap_hours(hours);
            self.skip_remaining = 0.0;
        }
    }

    /// Next landmark time strictly after now (06:00 / 12:00 / 18:00 / 24:00).
    pub fn next_phase(&self) -> f32 {
        const PHASES: [f32; 4] = [0.0, 6.0, 12.0, 18.0];
        for p in PHASES.iter() {
            if *p > self.hours + 1.0 / 512.0 {
                return *p;
            }
        }
        DAY_LENGTH
    }

    /// Animate towards [`SkyClock::next_phase`] over [`SKIP_SECONDS`] real seconds.
    pub fn skip_to_next_phase(&mut self) {
        if self.is_skipping() {
            return;
        }
        let target = self.next_phase();
        let dist = if target <= self.hours {
            target + DAY_LENGTH - self.hours
        } else {
            target - self.hours
        };
        self.skip_from = self.hours;
        self.skip_target = self.hours + dist;
        self.skip_remaining = SKIP_SECONDS;
    }

    /// Sampled [`SkySample`] of the current instant.
    pub fn sample(&self, sky: &Sky) -> SkySample {
        sky.sample(self.hours)
    }
}
