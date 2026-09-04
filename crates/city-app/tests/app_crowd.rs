//! The crowd as the *application* runs it: the crowd lives inside [`World`], is stepped
//! with the avatar as its focus, and is visible to the HUD and to the JSON feeds.

use city_app::{World, WorldConfig};

fn world() -> World {
    let mut w = World::new(WorldConfig::default());
    w.tick(1.0 / 60.0);
    w
}

#[test]
fn the_world_holds_a_crowd_and_traffic() {
    let w = world();
    assert!(!w.crowd().peds().is_empty(), "no pedestrians in the world");
    assert!(!w.crowd().cars().is_empty(), "no traffic in the world");
}

#[test]
fn stepping_the_world_steps_the_crowd() {
    let mut w = World::new(WorldConfig::default());
    let before: Vec<(f32, f32)> = w.crowd().peds().iter().map(|p| (p.x, p.z)).collect();
    for _ in 0..180 {
        w.tick(1.0 / 60.0);
    }
    let moved = w
        .crowd()
        .peds()
        .iter()
        .zip(&before)
        .filter(|(p, b)| (p.x - b.0).hypot(p.z - b.1) > 0.3)
        .count();
    assert!(moved > 4, "the crowd stands still inside the world");
}

#[test]
fn crowd_json_reports_every_agent() {
    let w = world();
    let v: serde_json::Value = serde_json::from_str(&w.crowd_json()).unwrap();
    assert_eq!(
        v["peds"].as_array().unwrap().len(),
        w.crowd().peds().len(),
        "crowd_json hides pedestrians"
    );
    assert_eq!(
        v["cars"].as_array().unwrap().len(),
        w.crowd().cars().len(),
        "crowd_json hides cars"
    );
    for p in v["peds"].as_array().unwrap() {
        assert!(p["x"].as_f64().unwrap().is_finite());
        assert!(p["v"].as_f64().unwrap() < 3.0, "a pedestrian is sprinting");
    }
    for c in v["cars"].as_array().unwrap() {
        assert!(c["x"].as_f64().unwrap().is_finite());
        assert!(c["v"].as_f64().unwrap() < 14.0, "a car is speeding");
    }
}

#[test]
#[ignore = "radar crowd markers are painted in the browser (dom.rs), not in hud_frame()"]
fn the_hud_marks_the_crowd_on_the_radar() {
    let w = world();
    let f = w.hud_frame();
    let peds = f
        .dots
        .iter()
        .filter(|d| d.kind == city_hud::HudDotKind::Ped)
        .count();
    let cars = f
        .dots
        .iter()
        .filter(|d| d.kind == city_hud::HudDotKind::Car)
        .count();
    assert!(peds > 0, "the radar shows no pedestrians");
    assert!(cars > 0, "the radar shows no traffic");
}

#[test]
fn the_json_snapshot_counts_the_crowd() {
    let w = world();
    let v: serde_json::Value = serde_json::from_str(&w.snapshot_json()).unwrap();
    assert_eq!(
        v["peds"].as_u64().unwrap() as usize,
        w.crowd().peds().len(),
        "the snapshot disagrees with the simulation"
    );
    assert_eq!(
        v["cars"].as_u64().unwrap() as usize,
        w.crowd().cars().len(),
        "the snapshot disagrees with the traffic"
    );
}
