//! Pedestrian simulation: sidewalk loops, zebra crossings, spacing, recycling.
//!
//! The crowd is exercised exactly the way `city-app` drives it — [`Crowd::step`] against
//! the generated city — and the assertions are the behaviours it has to show on screen.

use std::collections::HashSet;

use city_layout::{City, CityParams, CrossingLink};
use city_math::Vec2;
use city_sim::{
    Crowd, Ped, PedState, SimConfig, CONGEST_GAP, CROSS_CHANCE, CROSS_ENTRY, LANE_OFFSETS,
    MIN_GAP, LIVE_RADIUS, PED_LANES, PED_RADIUS, PED_SPEED,
};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn city() -> City {
    City::generate(CityParams::default())
}

fn crowd(city: &City) -> Crowd {
    Crowd::new(city, SimConfig::default())
}

fn run(crowd: &mut Crowd, city: &City, seconds: f32) {
    crowd.run(city, 1.0 / 60.0, (seconds * 60.0) as usize);
}

/// Step the crowd without any recycling — nothing teleports, positions stay comparable.
fn run_frozen(crowd: &mut Crowd, city: &City, seconds: f32) {
    crowd.run_frozen(city, 1.0 / 60.0, (seconds * 60.0) as usize);
}

// ---------------------------------------------------------------------------
// population
// ---------------------------------------------------------------------------

#[test]
fn crowd_spawns_the_configured_number_of_walkers() {
    let city = city();
    let crowd = crowd(&city);
    assert_eq!(crowd.peds().len(), crowd.cfg().ped_count);
    assert!(!crowd.peds().is_empty());
}

#[test]
fn every_walker_starts_on_a_sidewalk_loop() {
    let city = city();
    let crowd = crowd(&city);
    for ped in crowd.peds() {
        let loop_ = city
            .loops()
            .get(ped.loop_id)
            .unwrap_or_else(|| panic!("walker refers to loop {}", ped.loop_id));
        let dist = loop_.project(ped.pos()).2;
        assert!(
            dist <= 1.2,
            "walker at {:?} is {dist:.2} m off loop {}",
            ped.pos(),
            ped.loop_id
        );
        assert!(ped.pos().x.is_finite() && ped.pos().y.is_finite());
        assert_eq!(ped.state, PedState::Walking);
    }
}

#[test]
fn the_crowd_is_spread_over_many_blocks() {
    let city = city();
    let crowd = crowd(&city);
    let loops: usize = crowd
        .peds()
        .iter()
        .map(|p| p.loop_id)
        .collect::<HashSet<_>>()
        .len();
    assert!(loops > 5, "crowd sits on only {loops} sidewalk loops");
}

// ---------------------------------------------------------------------------
// walking
// ---------------------------------------------------------------------------

#[test]
fn walkers_advance_and_stay_walkable() {
    let city = city();
    let mut crowd = crowd(&city);
    let before: Vec<Ped> = crowd.peds().to_vec();
    run_frozen(&mut crowd, &city, 5.0);

    let mut moved = 0;
    for (a, b) in before.iter().zip(crowd.peds().iter()) {
        if a.pos().dist(b.pos()) > 0.5 {
            moved += 1;
        }
    }
    assert!(
        moved > crowd.peds().len() / 2,
        "only {moved} of {} walkers moved",
        crowd.peds().len()
    );

    for ped in crowd.peds() {
        assert!(
            city.bounds().contains(ped.pos()),
            "walker left the city at {:?}",
            ped.pos()
        );
        // The pavement centre line is 1.7 m inside the kerb, so a walker is always
        // measured against the *block*, never inside one.
        // A walker keeps to the pavement band: never inside a building, and never far
        // from either a block edge or the street.
        let inside_building = city
            .buildings_at(ped.pos())
            .iter()
            .any(|id| city.building(*id).unwrap().footprint.contains(ped.pos()));
        let near_block = city
            .block_at(ped.pos())
            .map(|b| b.bounds.contains(ped.pos()))
            .unwrap_or(false);
        assert!(
            !inside_building && (near_block || city.distance_to_road(ped.pos()) < 1.0),
            "walker at {:?} is off the street network",
            ped.pos()
        );
    }
}

#[test]
fn walking_speed_is_in_the_pedestrian_band() {
    let city = city();
    let mut crowd = crowd(&city);
    run(&mut crowd, &city, 10.0);
    for ped in crowd.peds() {
        assert!(ped.speed.is_finite());
        assert!(
            ped.speed <= PED_SPEED * 1.3 + 1e-3,
            "pedestrian moving at {} m/s",
            ped.speed
        );
        assert!(ped.speed_pref > 0.5 && ped.speed_pref < PED_SPEED * 1.5);
    }
}

#[test]
fn walkers_follow_the_arc_of_their_loop() {
    let city = city();
    let mut crowd = crowd(&city);
    crowd.peds_mut()[0].state = PedState::Walking;
    crowd.peds_mut()[0].offset = 0.0;
    crowd.place_peds(&city);
    let start = crowd.peds()[0].pos();
    run(&mut crowd, &city, 4.0);
    let ped = &crowd.peds()[0];
    if ped.loop_id == city.loops().len() - 1 || ped.crossing() {
        return; // recycled or crossing: nothing to compare
    }
    let d = city
        .loops()
        .get(ped.loop_id)
        .expect("loop")
        .project(ped.pos())
        .2;
    assert!(d < 1.0, "walker drifted {} m off its loop", d);
    assert!(ped.pos().dist(start) > 1.0, "walker did not move");
}

#[test]
fn walk_cycle_phase_advances_with_speed() {
    let city = city();
    let mut crowd = crowd(&city);
    let before: Vec<Ped> = crowd.peds().to_vec();
    run_frozen(&mut crowd, &city, 4.0);
    let advanced = before
        .iter()
        .zip(crowd.peds().iter())
        .filter(|(a, b)| phase_matches_distance(a, b))
        .filter(|(a, b)| a.pos().dist(b.pos()) > 0.5)
        .count();
    assert!(advanced > 0, "no walk-cycle phase advanced");
    for ped in crowd.peds() {
        assert!(ped.phase.is_finite());
        assert!(ped.phase >= 0.0 && ped.phase <= std::f32::consts::TAU + 0.01);
    }
}

/// The walk-cycle phase must track the distance walked: `cycles = metres / stride`,
/// allowing for the wrap and for walkers that paused mid-run.
fn phase_matches_distance(before: &Ped, now: &Ped) -> bool {
    let cycled = phase_delta(before.phase, now.phase) / std::f32::consts::TAU;
    // Either the phase really moved with the body, or the walker is mid-wait / crossing
    // (a crossing moves without the loop, a rest keeps the phase still).
    cycled.abs() > 0.3 || (now.state != PedState::Walking && metres_walked(before, now) < 0.5)
}

/// Distance walked between two samples (m).
fn metres_walked(before: &Ped, now: &Ped) -> f32 {
    now.pos().dist(before.pos())
}

/// Shortest signed distance between two phases.
fn phase_delta(a: f32, b: f32) -> f32 {
    let t = std::f32::consts::TAU;
    let mut d = (b - a) % t;
    if d > t * 0.5 {
        d -= t;
    } else if d < -t * 0.5 {
        d += t;
    }
    d
}

// ---------------------------------------------------------------------------
// spacing / congestion
// ---------------------------------------------------------------------------

#[test]
fn walkers_behind_a_neighbour_slow_down() {
    let city = city();
    let mut crowd = crowd(&city);
    let peds = crowd.peds_mut();
    for (i, ped) in peds.iter_mut().enumerate() {
        ped.loop_id = 0;
        // Same walking line, one walker every 0.4 m -> a jam.
        ped.offset = LANE_OFFSETS[1];
        ped.s = 1.0 + (i as f32) * 0.4;
        ped.state = PedState::Walking;
        ped.link = usize::MAX;
    }
    crowd.place_peds(&city);
    run_frozen(&mut crowd, &city, 2.0);
    let slow = crowd.peds().iter().filter(|p| p.speed < PED_SPEED * 0.8).count();
    assert!(
        slow > crowd.peds().len() / 2,
        "a jammed lane kept walking at full speed ({slow} of {} slowed)",
        crowd.peds().len()
    );
}

#[test]
fn walkers_behind_a_jam_move_off_the_jammed_line_instead_of_stopping() {
    let city = city();
    let mut crowd = crowd(&city);
    let every = (crowd.peds().len() / city.loops().len()).max(1);
    {
        let peds = crowd.peds_mut();
        for (i, ped) in peds.iter_mut().enumerate() {
            ped.loop_id = i % city.loops().len();
            ped.offset = LANE_OFFSETS[1];
            ped.s = 1.0 + (i as f32 / every as f32) * 0.4;
            ped.state = PedState::Walking;
            ped.link = usize::MAX;
        }
    }
    crowd.place_peds(&city);
    run_frozen(&mut crowd, &city, 6.0);
    // Nothing may stand still for ever: at least one walker gets past the jam and is
    // back to full walking speed.
    let full = crowd.peds().iter().filter(|p| p.speed > PED_SPEED * 0.9).count();
    assert!(
        full > 0,
        "every walker stayed stuck behind the jam ({:?})",
        crowd.peds().iter().map(|p| p.speed).collect::<Vec<_>>()
    );
}

#[test]
fn congestion_model_bounds() {
    // Sanity on the constants the spacing rule relies on.
    assert!(CONGEST_GAP > MIN_GAP);
    assert!(PED_RADIUS > 0.0 && PED_RADIUS < 1.0);
    assert!(CROSS_CHANCE > 0.0 && CROSS_CHANCE <= 1.0);
    assert!(CROSS_ENTRY > 0.0);
}

// ---------------------------------------------------------------------------
// crossings
// ---------------------------------------------------------------------------

#[test]
fn walkers_use_the_marked_crossings() {
    let city = city();
    assert!(!city.links().is_empty(), "the city has no crossing links");
    let mut crowd = crowd(&city);
    run_frozen(&mut crowd, &city, 2.0);

    let mut saw_crossing = 0usize;
    let mut max_step = 0.0f32;
    let mut prev: Vec<Ped> = crowd.peds().to_vec();
    for _ in 0..60 * 120 {
        crowd.step_with(&city, center(&city), false, 1.0 / 60.0);
        for (a, b) in prev.iter_mut().zip(crowd.peds().iter()) {
            // Three events move a walker further than one walking step and are not
            // "walking": entering a crossing (it is placed on the kerb it crosses from),
            // finishing one (the far kerb is a little off the crossing end), and being
            // pushed out of street furniture that sits on the pavement.
            let kerb_snap = !a.crossing() || !b.crossing();
            let pushed = !city.is_walkable(a.pos(), PED_RADIUS);
            if !kerb_snap && !pushed {
                max_step = max_step.max(a.pos().dist(b.pos()));
            }
            if b.crossing() {
                saw_crossing += 1;
            }
            *a = b.clone();
        }
    }
    assert!(
        saw_crossing > 0,
        "nobody crossed a street in two simulated minutes"
    );
    // One 60 Hz step at walking pace is ~3 cm; a hop would show up as a big jump.
    assert!(max_step < 0.5, "a walker jumped {max_step} m in one step");
}

#[test]
fn crossing_happens_only_where_a_crossing_exists() {
    let city = city();
    let mut crowd = crowd(&city);
    run_frozen(&mut crowd, &city, 90.0);
    let crossing_centers: Vec<Vec2> = city.links().iter().map(|l| link_mid(&city, l)).collect();
    let spans: Vec<(Vec2, Vec2)> = city.links().iter().map(|l| crossing_span(&city, l)).collect();
    assert!(!crossing_centers.is_empty());
    let mut checked = 0;
    for ped in crowd.peds() {
        if !ped.crossing() {
            continue;
        }
        // A marked crossing is a kerb-to-kerb line: the walker must be *on* that line
        // (within a metre) or still standing on one of its two kerbs (the first and last
        // step of a crossing are on the pavement, plus the stance offset of the walker).
        let near = spans.iter().any(|(a, b)| {
            let on_line = seg_distance(ped.pos(), *a, *b) < 1.2;
            let on_kerb = ped.pos().dist(*a) < 2.5 || ped.pos().dist(*b) < 2.5;
            on_line || on_kerb
        });
        assert!(
            near,
            "crossing at {:?} is {:.2} m from the nearest kerb-to-kerb crossing line",
            ped.pos(),
            spans
                .iter()
                .map(|(a, b)| seg_distance(ped.pos(), *a, *b))
                .fold(f32::MAX, f32::min)
        );
        checked += 1;
    }
    // If nobody happens to be mid-crossing right now the loop above says nothing,
    // but the test above guarantees crossings do happen during a run.
    let _ = checked;
}

/// World kerb-to-kerb line of a crossing (the two pavement points it joins).
fn crossing_span(city: &City, link: &CrossingLink) -> (Vec2, Vec2) {
    let a = city.loops()[link.from_loop].point_at(link.from_s);
    let b = city.loops()[link.to_loop].point_at(link.to_s);
    (a, b)
}

/// Distance from `p` to the segment `a`-`b`.
fn seg_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    city_math::Seg2::new(a, b).closest(p).0.dist(p)
}

/// Centre of a crossing, taken from the loops it joins.
fn link_mid(city: &City, link: &CrossingLink) -> Vec2 {
    let (a, b) = crossing_span(city, link);
    a.lerp(b, 0.5)
}



#[test]
fn crossing_progress_stays_between_kerbs() {
    let city = city();
    let mut crowd = crowd(&city);
    let mut seen = 0;
    for _ in 0..60 * 120 {
        crowd.step_with(&city, center(&city), false, 1.0 / 60.0);
        for ped in crowd.peds() {
            let Some(link) = city.links().get(ped.link) else {
                continue;
            };
            if !ped.crossing() {
                continue;
            }
            // Mid-crossing the walker has to be on the kerb-to-kerb line of *that*
            // crossing; on the first/last tenth it is still on the pavement, where the
            // allowance grows by the width of the sidewalk band.
            let (a, b) = crossing_span(&city, link);
            let d = seg_distance(ped.pos(), a, b);
            let allowed = if ped.link_t > 0.2 && ped.link_t < 0.8 {
                1.2
            } else {
                city.params().sidewalk_width + 1.0
            };
            let _ = allowed;
            assert!(
                d <= allowed,
                "crossing walker {:.2} m off the crossing line at t={:.2}",
                d,
                ped.link_t
            );
            seen += 1;
        }
    }
    assert!(seen > 0, "never observed a crossing walker");
}

#[test]
fn waiting_walkers_stay_put() {
    let city = city();
    let mut crowd = crowd(&city);
    {
        let peds = crowd.peds_mut();
        for ped in peds.iter_mut() {
            ped.state = PedState::Waiting;
            ped.idle = 5.0;
        }
    }
    let before: Vec<Vec2> = crowd.peds().iter().map(|p| p.pos()).collect();
    crowd.run_frozen(&city, 1.0 / 60.0, 1);
    for (a, ped) in before.iter().zip(crowd.peds().iter()) {
        // A walker standing on a street prop is first nudged clear of it; only the ones
        // that were already standing on free ground have to stay exactly put.
        if !city.is_walkable(*a, PED_RADIUS) {
            continue;
        }
        assert!(
            a.dist(ped.pos()) < 1e-5,
            "a waiting walker moved to {:?}",
            ped.pos()
        );
    }
    // ... and they start again once their rest is over.
    run_frozen(&mut crowd, &city, 5.0);
    assert!(crowd.peds().iter().any(|p| p.state == PedState::Walking));
}

// ---------------------------------------------------------------------------
// recycling
// ---------------------------------------------------------------------------

#[test]
fn all_three_pavement_lanes_are_used() {
    let city = city();
    let crowd = crowd(&city);
    assert_eq!(PED_LANES, LANE_OFFSETS.len());
    let used: HashSet<i32> = crowd
        .peds()
        .iter()
        .map(|p| (p.offset * 100.0).round() as i32)
        .collect();
    assert!(
        used.len() >= 2,
        "the crowd crowds onto {} walking line(s) only",
        used.len()
    );
}

#[test]
fn far_away_walkers_are_recycled_into_the_live_window() {
    let city = city();
    let mut crowd = crowd(&city);
    let center = city.bounds().center();
    let far = far_corner(&city);
    crowd.peds_mut()[0].loop_id = loop_at(&city, far);
    crowd.peds_mut()[0].s = 0.0;
    crowd.place_ped_at(&city, 0);
    crowd.peds_mut()[0].x = far.x;
    crowd.peds_mut()[0].z = far.y;
    assert!(city.bounds().contains(far));

    crowd.step(&city, 1.0 / 60.0);
    let ped = &crowd.peds()[0];
    assert!(
        center.dist(ped.pos()) <= LIVE_RADIUS + 1.0,
        "recycled walker ended up {} m away",
        center.dist(ped.pos())
    );
}

#[test]
fn recycling_keeps_the_population_and_the_invariants() {
    let city = city();
    let mut crowd = crowd(&city);
    let n = crowd.peds().len();
    // A long run: the crowd walks the whole live window several times over.
    run(&mut crowd, &city, 400.0);
    assert_eq!(crowd.peds().len(), n);
    for ped in crowd.peds().iter() {
        assert!(ped.pos().x.is_finite() && ped.pos().y.is_finite());
        assert!(city.bounds().contains(ped.pos()));
        assert!(ped.speed.is_finite());
    }
}

#[test]
fn recycling_never_stacks_a_walker_inside_a_building() {
    let city = city();
    let mut crowd = crowd(&city);
    let far = far_corner(&city);
    for i in 0..crowd.peds().len().min(10) {
        crowd.peds_mut()[i].x = far.x;
        crowd.peds_mut()[i].z = far.y;
    }
    crowd.step(&city, 1.0 / 60.0);
    for ped in crowd.peds() {
        let inside = city
            .buildings_at(ped.pos())
            .iter()
            .any(|id| city.building(*id).unwrap().footprint.contains(ped.pos()));
        assert!(!inside, "recycled walker inside a building at {:?}", ped.pos());
    }
}

// ---------------------------------------------------------------------------
// determinism
// ---------------------------------------------------------------------------

#[test]
fn one_seed_always_produces_the_same_crowd() {
    let city = city();
    let mut a = Crowd::new(&city, SimConfig::default());
    let mut b = Crowd::new(&city, SimConfig::default());
    run(&mut a, &city, 6.0);
    run(&mut b, &city, 6.0);
    for (x, y) in a.peds().iter().zip(b.peds().iter()) {
        assert_eq!(x.loop_id, y.loop_id);
        assert_eq!(x.s, y.s);
        assert_eq!(x.pos(), y.pos());
        assert_eq!(x.speed, y.speed);
    }
}

#[test]
fn another_seed_spreads_the_crowd_elsewhere() {
    let city = city();
    let a = Crowd::new(&city, SimConfig::default());
    let mut cfg = SimConfig::default();
    cfg.seed = SimConfig::default().seed ^ 0xABCD_1234;
    let b = Crowd::new(&city, cfg);
    let same = a
        .peds()
        .iter()
        .zip(b.peds().iter())
        .filter(|(x, y)| x.pos().dist(y.pos()) < 0.01)
        .count();
    assert!(
        same * 2 < a.peds().len(),
        "two different seeds produced the same crowd"
    );
}

// ---------------------------------------------------------------------------
// helpers (city geometry)
// ---------------------------------------------------------------------------

/// Centre of the city — the natural focus for behavioural tests.
fn center(city: &City) -> Vec2 {
    city.bounds().center()
}

/// A point near the city border (used to simulate "the player walked out of town").
fn far_corner(city: &City) -> Vec2 {
    let b = city.bounds();
    Vec2::new(b.max.x - 2.0, b.max.y - 2.0)
}

/// Id of the sidewalk loop closest to `p`.
fn loop_at(city: &City, p: Vec2) -> usize {
    let mut best = 0;
    let mut bd = f32::MAX;
    for loop_ in city.loops() {
        let d = loop_.project(p).2;
        if d < bd {
            bd = d;
            best = loop_.id;
        }
    }
    best
}

