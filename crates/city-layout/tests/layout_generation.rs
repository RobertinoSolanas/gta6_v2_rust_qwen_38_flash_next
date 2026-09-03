//! City generation invariants: grid counts, land use and block contents.

use city_layout::{Block, BlockKind, Building, City, CityParams, RoadKind};

fn default_city() -> City {
    City::generate(CityParams::default())
}

/// Mean building height over the blocks selected by `pick`.
fn mean_height(city: &City, pick: impl Fn(&Block) -> bool) -> f32 {
    let (sum, n) = city
        .blocks()
        .iter()
        .filter(|b| pick(b))
        .fold((0.0f32, 0usize), |acc, b| {
            let hs: Vec<f32> = b
                .buildings
                .iter()
                .filter_map(|id| city.building(*id))
                .map(|b: &Building| b.height)
                .collect();
            (acc.0 + hs.iter().sum::<f32>(), acc.1 + hs.len())
        });
    if n == 0 {
        0.0
    } else {
        sum / n as f32
    }
}

#[test]
fn default_city_has_content() {
    let city = default_city();
    assert_eq!(city.blocks().len(), 81);
    assert!(
        city.buildings().len() > 50,
        "buildings: {}",
        city.buildings().len()
    );
    assert!(!city.roads().is_empty());
    assert!(!city.crossings().is_empty());
    assert!(!city.links().is_empty());
    assert!(city.props().len() > 100, "props: {}", city.props().len());
}

#[test]
fn grid_arithmetic_is_consistent() {
    let params = CityParams {
        blocks_x: 6,
        blocks_z: 7,
        ..CityParams::default()
    };
    let city = City::generate(params.clone());
    assert_eq!(city.blocks().len(), 6 * 7);
    assert_eq!(city.grid(), [6, 7]);
    assert_eq!(city.intersections().len(), params.node_count());

    // Every carriageway carries exactly its two lanes, is never degenerate and
    // joins two distinct junctions inside the node range.
    for r in city.roads() {
        assert!(r.half_width > 0.0);
        assert!(r.lanes[1] == city.lanes()[r.lanes[0]].id + 1);
        assert_ne!(r.from_node, r.to_node);
        assert!(r.to_node < params.node_count());
        assert_eq!(city.lanes()[r.lanes[0]].road, r.id);
        assert_eq!(city.lanes()[r.lanes[1]].road, r.id);
    }
    // Exactly two lanes per carriageway, both directions of the same axis.
    assert_eq!(city.lanes().len(), city.roads().len() * 2);
    // The lane graph is wired up: every junction can be left and reached, and every
    // lane has at least one continuation.
    assert!(city
        .intersections()
        .iter()
        .all(|it| !it.arrivals.is_empty() && !it.departures.is_empty()));
    assert!(city.lanes().iter().all(|l| !l.next.is_empty()));
    // Some street lines are tagged as avenues.
    assert!(city.roads().iter().any(|r| r.kind == RoadKind::Avenue));
    assert!(city.roads().iter().any(|r| r.kind == RoadKind::Street));
}

#[test]
fn every_block_owns_a_sidewalk_loop() {
    let city = default_city();
    for block in city.blocks() {
        let ring = city
            .loops()
            .get(block.loop_index)
            .unwrap_or_else(|| panic!("block {:?} has no loop", block.cell));
        assert!(!ring.is_empty());
        assert!(ring.perimeter() > 100.0);
    }
}

#[test]
fn urban_blocks_are_built_up() {
    let city = default_city();
    let urban: Vec<&Block> = city
        .blocks()
        .iter()
        .filter(|b| b.kind == BlockKind::Urban)
        .collect();
    assert!(!urban.is_empty());
    for block in urban {
        assert!(
            !block.buildings.is_empty(),
            "urban block {:?} has no buildings",
            block.cell
        );
        for id in &block.buildings {
            let b = city.building(*id).expect("building id in range");
            assert!(b.height > 0.0);
            assert!(b.windows_x >= 1 && b.windows_z >= 1);
            assert!(b.footprint.size().x > 0.0 && b.footprint.size().y > 0.0);
        }
    }
}

#[test]
fn non_urban_blocks_have_no_buildings() {
    let city = default_city();
    for block in city.blocks() {
        if block.kind == BlockKind::Urban {
            continue;
        }
        assert!(
            block.buildings.is_empty(),
            "{:?} block {:?} carries buildings",
            block.kind,
            block.cell
        );
    }
}

#[test]
fn land_use_mix_is_represented() {
    let city = default_city();
    let count = |k: BlockKind| city.blocks().iter().filter(|b| b.kind == k).count();
    assert!(count(BlockKind::Urban) > city.blocks().len() / 2);
    assert!(count(BlockKind::Park) > 0, "expected at least one park");
}

#[test]
fn downtown_is_taller_than_the_edge() {
    let city = default_city();
    let core = mean_height(&city, |b| {
        b.cell[0] >= 3 && b.cell[0] <= 5 && b.cell[1] >= 3 && b.cell[1] <= 5
    });
    let rim = mean_height(&city, |b| b.edge);
    assert!(core > rim, "downtown {core} vs edge {rim}");
}

#[test]
fn bounds_match_params() {
    let params = CityParams::default();
    let city = City::generate(params.clone());
    let b = city.bounds();
    let expect = params.city_bounds();
    assert_eq!((b.min.x, b.min.y), (expect.min.x, expect.min.y));
    assert!((b.size().x - expect.size().x).abs() < 1e-3);
    assert_eq!(city.block_at_grid(0, 0).map(|k| k.cell), Some([0, 0]));
    assert!(city.block_at_grid(9, 0).is_none());
}
