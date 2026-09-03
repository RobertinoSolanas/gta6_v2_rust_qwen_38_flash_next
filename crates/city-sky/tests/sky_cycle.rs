//! Day/night cycle behaviour of `city-sky`: sun/moon geometry, phase naming,
//! colour band ordering, and every scalar curve the renderer/sim depend on.

use city_sky::*;

const AZ: f32 = DEFAULT_AZIMUTH;

fn sky() -> Sky {
    Sky::new(AZ)
}

fn eq(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() < eps
}

/// Horizontal heading of a direction vector (used to compare sun arcs).
fn yaw_of(v: city_math::Vec3) -> f32 {
    v.z.atan2(v.x)
}

// ---------------------------------------------------------------------------
// sun & moon geometry
// ---------------------------------------------------------------------------

#[test]
fn sun_arc_rises_and_sets() {
    // arc definition: sunrise 06:00, peak ~12:00, sunset 18:00
    assert!(
        eq(sun_elevation_deg(6.0, AZ), 0.0, 0.6),
        "sunrise at 06:00, got {}",
        sun_elevation_deg(6.0, AZ)
    );
    assert!(eq(sun_elevation_deg(18.0, AZ), 0.0, 0.6));
    assert!(sun_elevation_deg(12.0, AZ) > 50.0, "high sun at noon");
    assert!(sun_elevation_deg(0.0, AZ) < -25.0, "midnight is deep night");
    assert!(sun_elevation_deg(3.0, AZ) < -20.0);
    assert!(sun_elevation_deg(9.0, AZ) > 40.0);
}

#[test]
fn sun_vector_is_unit_everywhere() {
    for i in 0..=96 {
        let h = i as f32 * 0.25;
        let d = sun_dir(h, AZ);
        assert!(eq(d.len(), 1.0, 1e-5), "sun not unit at {h}: {d:?}");
        let m = moon_dir(h, AZ);
        assert!(eq(m.len(), 1.0, 1e-5));
        assert!(d.dot(m) < -0.999, "moon must be opposite the sun");
    }
}

#[test]
fn day_night_flags_agree_with_elevation() {
    assert!(is_daytime(12.0, AZ));
    assert!(!is_daytime(0.0, AZ));
    assert!(is_daytime(7.0, AZ));
    assert!(!is_daytime(19.0, AZ));
    let noon = sky().sample(12.0);
    let night = sky().sample(1.0);
    assert!(noon.is_daytime() && !night.is_daytime());
}

#[test]
fn azimuth_rotates_the_arc_but_not_the_schedule() {
    let a = sun_dir(7.0, 0.0);
    let b = sun_dir(7.0, std::f32::consts::FRAC_PI_2);
    assert!(eq(a.y, b.y, 1e-5), "elevation schedule is azimuth free");
    let diff = (yaw_of(a) - yaw_of(b)).abs();
    assert!(diff > 1.0, "azimuth must rotate the sun path ({diff})");
}

// ---------------------------------------------------------------------------
// phases
// ---------------------------------------------------------------------------

#[test]
fn phase_names_across_a_day() {
    let s = sky();
    assert_eq!(s.sample(12.0).phase(), "day");
    assert_eq!(s.sample(4.0).phase(), "night");
    assert_eq!(s.sample(5.6).phase(), "dawn");
    assert_eq!(s.sample(7.0).phase(), "morning");
    assert_eq!(s.sample(17.4).phase(), "evening");
    assert_eq!(s.sample(18.4).phase(), "dusk");
}

#[test]
fn transition_band_brackets_the_horizon() {
    let s = sky();
    assert!(s.sample(5.9).is_transition());
    assert!(s.sample(18.1).is_transition());
    assert!(!s.sample(12.0).is_transition());
    assert!(!s.sample(1.0).is_transition());
}

// ---------------------------------------------------------------------------
// colours
// ---------------------------------------------------------------------------

#[test]
fn sky_is_blue_day_pale_horizon() {
    let s = sky().sample(12.0);
    assert!(
        s.zenith[2] > s.zenith[0] * 3.0,
        "day zenith: {:?}",
        s.zenith
    );
    assert!(s.zenith[2] > 0.7 && s.zenith[0] < 0.25);
    assert!(s.horizon[2] > s.horizon[0], "horizon: {:?}", s.horizon);
    assert!(s.horizon[2] > 0.8 && s.horizon[2] < 1.0);
    assert!(s.horizon[0] > s.zenith[0], "horizon paler than zenith");
}

#[test]
fn night_is_dark_and_blue() {
    let s = sky().sample(1.0);
    assert!(
        s.zenith.iter().all(|c| *c < 0.06),
        "night zenith: {:?}",
        s.zenith
    );
    assert!(s.zenith[2] > s.zenith[0]);
    assert!(s.horizon[0] < 0.1);
    assert!(s.glow_strength < 0.01, "no glow at deep night");
}

/// Maximum glow strength in a +-0.2 h window around `hours`.
fn glow_peak(sky: &Sky, hours: f32) -> f32 {
    (0..=80)
        .map(|i| sky.sample(hours + (i - 40) as f32 * 0.005).glow_strength)
        .fold(0.0, f32::max)
}

#[test]
fn sunset_glow_peaks_at_the_horizon() {
    let s = sky();
    assert!(glow_peak(&s, 6.0) > 0.95, "glow must peak at sunrise");
    assert!(glow_peak(&s, 18.0) > 0.95, "glow must peak at sunset");
    assert!(glow_strength(30.0) < 0.01, "no glow with a high sun");
    assert!(glow_peak(&s, 1.0) < 0.01, "no glow deep at night");
    let g = s.sample(18.0).glow;
    assert!(g[0] > g[1] && g[1] >= g[2], "glow is warm: {g:?}");
}

#[test]
fn zenith_brightens_monotonically_towards_noon() {
    let s = sky();
    let lum = |c: [f32; 3]| 0.25 * c[0] + 0.6 * c[1] + 0.15 * c[2];
    let mut prev = -1.0;
    for i in 0..=40 {
        let h = 6.0 + i as f32 * 0.15; // 06:00 .. 12:00
        let z = lum(s.sample(h).zenith);
        assert!(z >= prev - 1e-5, "zenith darkened at {h}");
        prev = z;
    }
}

#[test]
fn light_color_is_day_warm_and_night_cool() {
    let s = sky();
    let noon = s.sample(12.0);
    let night = s.sample(1.0);
    assert!(noon.light_strength > 0.9);
    assert!(noon.light_color[0] > noon.light_color[2]);
    assert_eq!(
        night.light_strength, 0.0,
        "moon carries no directional strength"
    );
    assert!(
        night.light_color[2] > night.light_color[0],
        "moonlight is cool: {:?}",
        night.light_color
    );
    assert!(night.light_color[2] > 0.0, "but never pitch black");
}

// ---------------------------------------------------------------------------
// fog & exposure
// ---------------------------------------------------------------------------

#[test]
fn fog_matches_horizon_and_shortens_at_night() {
    let s = sky();
    let day = s.sample(12.0);
    let night = s.sample(1.0);
    assert!(day.fog[2] > day.fog[0]);
    assert!(day.fog_distance > night.fog_distance);
    assert!(day.fog_distance > 300.0 && day.fog_distance < 700.0);
    assert!(night.fog_distance > 100.0);
    let d = (day.fog[2] - day.horizon[2]).abs();
    assert!(d < 0.25, "fog detached from horizon: {d}");
}

#[test]
fn exposure_opens_up_at_night() {
    let s = sky();
    let day = s.sample(12.0);
    let dusk = s.sample(18.4);
    let night = s.sample(1.0);
    assert!(eq(day.exposure, 1.0, 1e-4));
    assert!(night.exposure > day.exposure * 1.8);
    assert!(dusk.exposure > day.exposure && dusk.exposure < night.exposure);
}

// ---------------------------------------------------------------------------
// curves
// ---------------------------------------------------------------------------

#[test]
fn daylight_curve_is_sane() {
    assert!(daylight(20.0) > 0.99);
    assert!(daylight(-20.0) < 0.01);
    assert!(daylight(0.0) > 0.05 && daylight(0.0) < 0.4);
}

#[test]
fn stars_only_show_at_night() {
    assert!(star_fade(-20.0) > 0.99);
    assert!(star_fade(10.0) < 0.01);
    assert!(star_fade(-2.0) > 0.4 && star_fade(-2.0) < 0.95);
}

#[test]
fn artificial_lights_on_at_night_off_by_day() {
    let e_night = sun_elevation_deg(1.0, AZ);
    let e_noon = sun_elevation_deg(12.0, AZ);
    assert!(window_light(e_night) > 0.99);
    assert!(lamp_light(e_night) > 0.99);
    assert!(headlight(e_night) > 0.99);
    assert!(window_light(e_noon) < 0.02);
    assert!(lamp_light(e_noon) < 0.02);
    assert!(headlight(e_noon) < 0.02);
}

#[test]
fn lamps_and_headlights_flicker_on_around_dusk() {
    let s = sky();
    let dusk = s.sample(19.5);
    let day = s.sample(11.0);
    assert!(dusk.lamp_light > day.lamp_light + 0.5);
    assert!(dusk.window_light > day.window_light);
    assert!(dusk.headlight > 0.8 && day.headlight < 0.02);
}

#[test]
fn evening_windows_are_lit_before_the_deep_night_plateau() {
    let e_evening = sun_elevation_deg(20.0, AZ); // ~2 hours after sunset
    assert!(
        window_light(e_evening) > 0.5,
        "dusk windows should already be lit (elev {})",
        e_evening
    );
    assert!(lamp_light(e_evening) > 0.6);
    assert!(window_light(sun_elevation_deg(1.0, AZ)) > 0.99);
}

#[test]
fn ambient_never_zero_and_grows_with_day() {
    let s = sky();
    let night = s.sample(1.0);
    let day = s.sample(12.0);
    assert!(night.ambient > 0.0, "must not go pitch black");
    assert!(day.ambient > night.ambient * 5.0);
    assert!(day.ambient <= 1.0);
}

// ---------------------------------------------------------------------------
// clock & time handling
// ---------------------------------------------------------------------------

#[test]
fn wrap_hours_wraps_both_directions() {
    assert!(eq(wrap_hours(24.0), 0.0, 1e-4));
    assert!(eq(wrap_hours(-0.5), 23.5, 1e-4));
    assert!(eq(wrap_hours(30.0), 6.0, 1e-4));
    assert!(eq(wrap_hours(-25.0), 23.0, 1e-4));
    assert!(wrap_hours(23.9) > 23.0);
}

#[test]
fn format_clock_is_hh_mm() {
    assert_eq!(format_clock(0.0), "00:00");
    assert_eq!(format_clock(7.5), "07:30");
    assert_eq!(format_clock(23.9), "23:54");
    assert_eq!(
        format_clock(11.999),
        "12:00",
        "59:60 rolls into the next hour"
    );
    assert_eq!(format_clock(-0.5), "23:30");
}

#[test]
fn clock_advances_and_wraps() {
    let mut c = SkyClock::new(23.9, 1.0); // 1 sim-hour per real second
    assert_eq!(c.clock(), "23:54");
    c.advance(0.02);
    assert!(c.hours() > 23.9);
    c.advance(1.0); // +1 h -> wraps past midnight
    assert!(c.hours() < 1.0, "wrapped: {}", c.hours());
    assert!(c.hours() > 0.4);
}

#[test]
fn clock_ignores_garbage_and_negative_dt() {
    let mut c = SkyClock::new(9.0, 1.0);
    c.advance(-1.0);
    c.advance(f32::NAN);
    c.set_hours(f32::NAN);
    assert!(eq(c.hours(), 9.0, 1e-5));
    c.advance(1.0);
    assert!(eq(c.hours(), 10.0, 1e-4));
}

#[test]
fn next_phase_lists_the_landmarks() {
    assert!(eq(SkyClock::new(3.0, 1.0).next_phase(), 6.0, 1e-5));
    assert!(eq(SkyClock::new(6.0, 1.0).next_phase(), 12.0, 1e-5));
    assert!(eq(SkyClock::new(12.5, 1.0).next_phase(), 18.0, 1e-5));
    assert!(eq(SkyClock::new(20.0, 1.0).next_phase(), 24.0, 1e-5));
    assert!(eq(SkyClock::new(23.99, 1.0).next_phase(), 24.0, 1e-5));
}

#[test]
fn skip_to_next_phase_animates_then_lands_on_it() {
    let mut c = SkyClock::new(17.0, 0.1);
    c.skip_to_next_phase();
    assert!(c.is_skipping());
    let mut guard = 0;
    while c.is_skipping() && guard < 1000 {
        c.advance(0.05);
        guard += 1;
    }
    assert!(!c.is_skipping());
    assert!(
        eq(c.hours(), 18.0, 1e-3),
        "skip landed at {} after {guard} steps",
        c.hours()
    );
    c.advance(1.0);
    assert!(c.hours() > 18.0 && c.hours() < 19.0);
}

#[test]
fn skip_wraps_past_midnight() {
    let mut c = SkyClock::new(23.0, 0.0);
    c.skip_to_next_phase();
    let mut guard = 0;
    while c.is_skipping() && guard < 1000 {
        c.advance(0.05);
        guard += 1;
    }
    assert!(eq(c.hours(), 0.0, 1e-3), "got {}", c.hours());
}

#[test]
fn skip_is_ignored_while_already_skipping() {
    let mut c = SkyClock::new(5.0, 0.0);
    c.skip_to_next_phase();
    let first = c.hours();
    c.skip_to_next_phase();
    assert!(eq(c.hours(), first, 1e-5));
}

// ---------------------------------------------------------------------------
// determinism / continuity
// ---------------------------------------------------------------------------

#[test]
fn sampling_is_deterministic_and_periodic() {
    let s = sky();
    for i in 0..50 {
        let h = i as f32 * 0.47;
        assert_eq!(s.sample(h), s.sample(h));
        let a = s.sample(h);
        let b = s.sample(h + 24.0);
        let d = (a.sun_elev_deg - b.sun_elev_deg).abs() + (a.exposure - b.exposure).abs();
        assert!(d < 1e-3, "not periodic at {h}: {d}");
        assert!(s.sample(h - 48.0).sun_elev_deg - a.sun_elev_deg < 1e-3);
    }
}

#[test]
fn sample_is_continuous_across_the_day_boundary() {
    let s = Sky::new(AZ);
    let a = s.sample(23.995);
    let b = s.sample(0.005);
    let d = (a.zenith[0] - b.zenith[0]).abs()
        + (a.fog[1] - b.fog[1]).abs()
        + (a.ambient - b.ambient).abs();
    assert!(d < 0.02, "midnight discontinuity: {d}");
}

#[test]
fn defaults_start_in_the_morning() {
    let c = SkyClock::default();
    assert!(c.hours() > 6.0 && c.hours() < 9.0);
    assert!(c.scale() > 0.0);
    assert!(Sky::default().sample(12.0).is_daytime());
}
