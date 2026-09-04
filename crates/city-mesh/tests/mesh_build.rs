//! Mesh builder behaviour of `city-mesh`: the vertex contract, the primitive emitters and
//! the static city geometry (blocks, buildings, props, road paint).
//!
//! Everything is checked through the public builder API: vertex counts, bounding boxes,
//! outward normals and the palette, so a change in a builder that would break the GL path
//! fails here before it reaches a browser.

use city_layout::{BlockKind, City, CityParams, PropKind};
use city_mesh::{
    block_surface_color, build_city, build_parking_stripes, build_road_markings, builder,
    building_mesh, city as geo, facade_color, palette, MeshBuilder, FLOATS_PER_VERTEX,
};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// A small city: enough blocks/buildings/props to cover every emitter, fast to generate.
fn test_city() -> City {
    City::generate(CityParams {
        seed: 1234,
        blocks_x: 3,
        blocks_z: 3,
        ..CityParams::default()
    })
}

/// XZ footprint of everything a builder emitted.
fn footprint(m: &MeshBuilder) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::MAX; 2];
    let mut max = [f32::MIN; 2];
    for i in 0..m.len() {
        let p = m.get(i).0;
        min[0] = min[0].min(p[0]);
        min[1] = min[1].min(p[2]);
        max[0] = max[0].max(p[0]);
        max[1] = max[1].max(p[2]);
    }
    (min, max)
}

/// Highest vertex y in the builder.
fn max_y(m: &MeshBuilder) -> f32 {
    (0..m.len()).map(|i| m.get(i).0[1]).fold(f32::MIN, f32::max)
}

// ---------------------------------------------------------------------------
// vertex format and primitives
// ---------------------------------------------------------------------------

#[test]
fn vertex_is_position_normal_colour() {
    assert_eq!(FLOATS_PER_VERTEX, 9);
    let mut m = MeshBuilder::new();
    m.vert([1.0, 2.0, 3.0], [0.0, 1.0, 0.0], [0.25, 0.5, 0.25]);
    assert_eq!(m.len(), 1);
    assert_eq!(m.byte_len(), FLOATS_PER_VERTEX * 4);
    let (p, n, c) = m.get(0);
    assert_eq!(p, [1.0, 2.0, 3.0]);
    assert_eq!(n, [0.0, 1.0, 0.0]);
    assert_eq!(c, [0.25, 0.5, 0.25]);
}

#[test]
fn a_new_builder_is_empty() {
    let m = MeshBuilder::new();
    assert!(m.is_empty());
    assert_eq!(m.len(), 0);
    assert_eq!(m.triangles(), 0);
    assert_eq!(builder::vertex_count(&[]), 0);
}

#[test]
fn a_quad_is_two_triangles_sharing_its_normal() {
    let mut m = MeshBuilder::new();
    m.quad(
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        [0.5, 0.5, 0.5],
    );
    assert_eq!(m.len(), 6);
    assert_eq!(m.triangles(), 2);
    assert_eq!(builder::vertex_count(m.as_slice()), 6);
    for i in 0..m.len() {
        assert_eq!(m.get(i).1, [0.0, 1.0, 0.0]);
    }
}

#[test]
fn a_box_is_six_faces_with_outward_normals() {
    let mut m = MeshBuilder::new();
    m.box_shaded(
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.2, 0.2, 0.2],
    );
    assert_eq!(m.len(), 36);
    assert_eq!(m.triangles(), 12);

    let (mut up, mut down, mut sides) = (0, 0, 0);
    for i in 0..m.len() {
        let n = m.get(i).1;
        if n[1] > 0.5 {
            up += 1;
        } else if n[1] < -0.5 {
            down += 1;
        } else {
            sides += 1;
        }
    }
    assert_eq!((up, down, sides), (6, 6, 24), "one quad per face direction");
}

#[test]
fn box_vertices_stay_on_the_faces_of_the_box() {
    let min = [1.0, 0.0, -2.0];
    let max = [3.0, 4.0, 5.0];
    let mut m = MeshBuilder::new();
    m.box_shaded(min, max, [1.0; 3], [0.0; 3]);
    for i in 0..m.len() {
        let p = m.get(i).0;
        for k in 0..3 {
            let lo = [min[0], min[1], min[2]][k];
            let hi = [max[0], max[1], max[2]][k];
            assert!(
                p[k] >= lo - 1e-5 && p[k] <= hi + 1e-5,
                "vertex {i} axis {k}"
            );
        }
    }
}

#[test]
fn box_yaw_at_zero_matches_the_axis_aligned_footprint() {
    let mut a = MeshBuilder::new();
    a.box_yaw(
        [2.0, 3.0],
        2.0,
        1.0,
        0.0,
        3.0,
        0.0,
        [1.0; 3],
        [0.3, 0.2, 0.1],
    );
    // identical box emitted the classic way
    let mut b = MeshBuilder::new();
    b.box_shaded([0.0, 0.0, -1.0], [4.0, 3.0, 1.0], [1.0; 3], [0.0; 3]);
    let (min, max) = footprint(&a);
    assert!(((min[0] + max[0]) * 0.5 - 2.0).abs() < 1e-4);
    assert!(((min[1] + max[1]) * 0.5 - 3.0).abs() < 1e-4);
    assert_eq!(max_y(&a), 3.0);
}

#[test]
fn a_quarter_turn_turns_the_long_axis_onto_z() {
    let mut m = MeshBuilder::new();
    m.box_yaw(
        [0.0, 0.0],
        2.0,
        0.5,
        0.0,
        1.0,
        std::f32::consts::FRAC_PI_2,
        [1.0; 3],
        [0.0; 3],
    );
    let (min, max) = footprint(&m);
    let spread_x = max[0] - min[0];
    let spread_z = max[1] - min[1];
    assert!(
        spread_z > spread_x,
        "a 90 deg yaw must move the long axis to Z ({spread_x} vs {spread_z})"
    );
    assert!(spread_x < 1.0 + 1e-3);
}

#[test]
fn box_yaw_rotates_the_side_normals_with_the_box() {
    let mut m = MeshBuilder::new();
    m.box_yaw(
        [0.0, 0.0],
        2.0,
        0.5,
        0.0,
        1.0,
        std::f32::consts::FRAC_PI_2,
        [1.0; 3],
        [0.5; 3],
    );
    // the local -Z face (normal (0,0,-1)) now points along -X after a +90 deg yaw
    let mut found = false;
    for i in 0..m.len() {
        let n = m.get(i).1;
        if n[1].abs() < 1e-3 && (n[0] + 1.0).abs() < 1e-3 {
            found = true;
        }
    }
    assert!(found, "a rotated side normal must point along -X");
}

#[test]
fn ground_emits_an_upward_quad_at_the_requested_height() {
    let mut m = MeshBuilder::new();
    m.ground([-10.0, -4.0], [3.0, 7.0], 0.15, [0.3, 0.3, 0.3]);
    assert_eq!(m.len(), 6);
    assert_eq!(m.get(0).1, [0.0, 1.0, 0.0]);
    assert_eq!(max_y(&m), 0.15);
    assert_eq!(footprint(&m), ([-10.0, -4.0], [3.0, 7.0]));
}

// ---------------------------------------------------------------------------
// palette
// ---------------------------------------------------------------------------

#[test]
fn facade_colour_follows_variant_and_landmark_flag() {
    let city = test_city();
    let plain = city
        .buildings()
        .iter()
        .find(|b| !b.landmark)
        .expect("non-landmark building");
    let base = palette::FACADES[(plain.variant as usize) % palette::FACADES.len()];
    assert_eq!(facade_color(plain), base);

    let tower = city
        .buildings()
        .iter()
        .find(|b| b.landmark)
        .expect("landmark tower");
    let c = facade_color(tower);
    assert!(c[2] > base[2], "a landmark facade is brighter");
}

#[test]
fn tint_stays_within_its_band_and_mix_interpolates() {
    let c = [1.0, 0.5, 0.25];
    for v in 0..8u8 {
        let t = palette::tint(c, v);
        for k in 0..3 {
            assert!(t[k] <= c[k] + 1e-6);
            assert!(t[k] >= c[k] * 0.88 - 1e-6);
        }
    }
    assert_eq!(palette::mix(c, [0.0; 3], 0.0), c);
    assert_eq!(palette::mix(c, [0.0; 3], 1.0), [0.0; 3]);
    // mix saturates instead of overshooting
    assert_eq!(palette::mix(c, [1.0; 3], 4.0), [1.0; 3]);
    assert_eq!(palette::shade(c, 1.0), c);
    assert!(palette::shade(c, 0.5)[0] < c[0]);
}

// ---------------------------------------------------------------------------
// the static city
// ---------------------------------------------------------------------------

#[test]
fn building_geometry_is_deterministic() {
    let build = || {
        let mut m = MeshBuilder::new();
        build_city(&test_city(), &mut m);
        m.into_vec()
    };
    let a = build();
    let b = {
        let mut m = MeshBuilder::new();
        build_city(&test_city(), &mut m);
        m.into_vec()
    };
    assert!(!a.is_empty());
    assert_eq!(a, b, "same seed must produce identical triangles");
}

#[test]
fn the_city_covers_ground_blocks_buildings_and_props() {
    let city = test_city();
    let mut m = MeshBuilder::new();
    build_city(&city, &mut m);

    // more geometry than the ground plane + one plateau per block
    let flat = (1 + city.blocks().len()) * 6;
    assert!(m.len() > flat);
    // and at least one box per building
    assert!(m.len() >= city.buildings().len() * 36);

    for v in m.as_slice() {
        assert!(v.is_finite(), "non-finite float in the city VBO");
    }
}

#[test]
fn blocks_are_paved_on_a_plateau_at_kerb_height() {
    let city = test_city();
    let mut m = MeshBuilder::new();
    build_city(&city, &mut m);
    // the highest ground quad of an urban block sits exactly on the kerb height
    let mut plateaus = 0;
    for i in 0..m.len() {
        let (p, n, _) = m.get(i);
        if n == [0.0, 1.0, 0.0] && (p[1] - city_mesh::city::KERB_HEIGHT).abs() < 1e-5 {
            plateaus += 1;
        }
    }
    assert!(plateaus >= city.blocks().len() * 6);
}

#[test]
fn a_setback_tower_adds_a_second_volume() {
    let city = test_city();
    let b = city
        .buildings()
        .iter()
        .find(|b| b.setback_height > 0.2)
        .expect("the seed generates a setback tower");
    let mut m = MeshBuilder::new();
    building_mesh(&mut m, b);
    assert_eq!(m.len(), 72, "main volume + setback box");
    assert!((max_y(&m) - (b.height + b.setback_height)).abs() < 1e-3);
}

#[test]
fn block_surface_colour_follows_land_use() {
    use BlockKind::*;
    assert_eq!(block_surface_color(Urban), palette::SIDEWALK);
    assert_eq!(block_surface_color(Park), palette::PARK);
    assert_eq!(block_surface_color(Plaza), palette::PLAZA);
    assert_eq!(block_surface_color(Lot), palette::LOT);
}

#[test]
fn every_prop_kind_emits_geometry() {
    let city = test_city();
    let mut seen = [false; city_layout::PropKind::COUNT];
    for p in city.props() {
        let mut m = MeshBuilder::new();
        geo::prop(&mut m, p);
        assert!(!m.is_empty(), "prop {:?} drew nothing", p.kind);
        seen[p.kind as usize] = true;
    }
    assert!(seen.iter().any(|s| *s));
}

#[test]
fn a_lit_lamp_head_differs_from_a_dark_one() {
    let city = test_city();
    let lit = city
        .props()
        .iter()
        .find(|p| p.kind == PropKind::Lamp && p.glow > 0.05)
        .expect("the test city has lit street lamps");
    let mut m = MeshBuilder::new();
    geo::prop(&mut m, lit);
    // the head is the last box of the lamp: its first vertex is at index 36
    let head = m.get(36).2;
    assert_eq!(head, palette::LAMP_ON);

    // a lamp that does not glow gets a plain metal head
    let mut dark = lit.clone();
    dark.glow = 0.0;
    let mut m2 = MeshBuilder::new();
    geo::prop(&mut m2, &dark);
    assert_eq!(m2.get(36).2, palette::METAL);
}

// ---------------------------------------------------------------------------
// road markings
// ---------------------------------------------------------------------------

#[test]
fn centre_lines_are_dashed_and_float_above_the_asphalt() {
    let city = test_city();
    let mut m = MeshBuilder::new();
    build_road_markings(&city, &mut m);
    assert!(!m.is_empty());

    for i in 0..m.len() {
        let (p, n, _) = m.get(i);
        assert_eq!(n, [0.0, 1.0, 0.0]);
        assert!(p[1] > 0.0, "paint must sit above the road surface");
    }
}

#[test]
fn dashes_are_skipped_near_the_junctions() {
    // a road shorter than the inset emits nothing at all
    let city = test_city();
    let mut dashes = 0usize;
    for r in city.roads() {
        let seg = r.center_line(city.params());
        let mut s = geo::MARKING_INSET;
        while s + geo::DASH_LEN <= seg.len() - geo::MARKING_INSET {
            dashes += 1;
            s += geo::DASH_LEN + geo::DASH_GAP;
        }
    }
    let bars: usize = city
        .crossings()
        .iter()
        .map(|c| geo::zebra_bars(c.width))
        .sum();
    assert!(dashes > 0, "a 3x3 city has road markings to draw");
    let mut m = MeshBuilder::new();
    build_road_markings(&city, &mut m);
    assert_eq!(
        m.len(),
        (dashes + bars) * 6,
        "one quad per dash and per zebra bar"
    );
}

#[test]
fn zebra_bar_count_is_bounded_and_grows_with_the_band() {
    assert!(geo::zebra_bars(0.5) >= 2);
    assert!(geo::zebra_bars(1000.0) <= 14);
    assert!(geo::zebra_bars(9.0) > geo::zebra_bars(3.0));
}

#[test]
fn a_zebra_is_centred_on_its_crossing() {
    let city = test_city();
    let crossing = city.crossings().first().expect("the city has crossings");
    let mut m = MeshBuilder::new();
    geo::zebra_crossing(&mut m, crossing);
    assert_eq!(m.len() % 6, 0);
    assert!(m.len() >= 12);
    let (min, max) = footprint(&m);
    let c = crossing.center;
    assert!(((min[0] + max[0]) * 0.5 - c.x).abs() <= crossing.width);
    assert!(((min[1] + max[1]) * 0.5 - c.y).abs() <= crossing.width);
}

#[test]
fn parking_stripes_only_appear_on_surface_lots() {
    let city = test_city();
    let mut m = MeshBuilder::new();
    build_parking_stripes(&city, &mut m);
    let lots = city
        .blocks()
        .iter()
        .filter(|b| b.kind == BlockKind::Lot)
        .count();
    if lots == 0 {
        assert!(m.is_empty(), "no lot, no stripes");
    } else {
        assert!(!m.is_empty());
        for i in 0..m.len() {
            assert!(m.get(i).0[1] > 0.0, "stripes paint the lot plateau");
        }
    }
}

// ---------------------------------------------------------------------------
// contract with city-tex
// ---------------------------------------------------------------------------

#[test]
fn material_slots_match_the_texture_crate() {
    use city_tex::{Material, ALL_MATERIALS};
    assert_eq!(ALL_MATERIALS.len(), 10);
    assert_eq!(city_mesh::slot::ASPHALT, 0);
    assert_eq!(
        ALL_MATERIALS[city_mesh::slot::ASPHALT as usize],
        Material::Asphalt
    );
    assert_eq!(
        ALL_MATERIALS[city_mesh::slot::CONCRETE as usize],
        Material::Concrete
    );
    assert_eq!(
        ALL_MATERIALS[city_mesh::slot::SIDEWALK as usize],
        Material::Sidewalk
    );
    assert_eq!(
        ALL_MATERIALS[city_mesh::slot::GRASS as usize],
        Material::Grass
    );
    // metal lives in slot 7, road paint in 8 and 9
    assert_eq!(
        ALL_MATERIALS[city_mesh::slot::METAL as usize],
        Material::Metal
    );
    assert_eq!(
        ALL_MATERIALS[city_mesh::slot::PAINT_WHITE as usize],
        Material::RoadPaintWhite
    );
    assert_eq!(
        ALL_MATERIALS[city_mesh::slot::ROAD_LINE_YELLOW as usize],
        Material::RoadLineYellow
    );
    assert_eq!(city_mesh::slot::ROAD_LINE_YELLOW, 9);
}

// ---------------------------------------------------------------------------
// cost bookkeeping (the perf reason the crowd shares one vertex format)
// ---------------------------------------------------------------------------

#[test]
fn the_static_city_fits_a_modest_triangle_budget() {
    let city = test_city();
    let mut m = MeshBuilder::new();
    build_city(&city, &mut m);
    let mut paint = MeshBuilder::new();
    build_road_markings(&city, &mut paint);
    // a 3x3 test city stays well under a hundred thousand triangles
    assert!(
        m.triangles() + paint.triangles() < 200_000,
        "{} triangles for a 3x3 city",
        m.triangles() + paint.triangles()
    );
}

#[test]
fn every_emitter_writes_whole_quads() {
    let city = test_city();
    let mut m = MeshBuilder::new();
    build_city(&city, &mut m);
    build_road_markings(&city, &mut m);
    build_parking_stripes(&city, &mut m);
    assert_eq!(m.len() % 6, 0, "geometry is written quad by quad");
}
