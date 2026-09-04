//! Static city geometry: ground, kerbs, buildings, street furniture and road markings.
//!
//! Everything here is a pure function of the [`City`] the layout context generated: same
//! seed, identical triangle soup. The builders are deliberately dumb — they read the
//! generated layout and emit boxes and quads; they never decide anything about the city.

use city_layout::{City, PropKind, RoadKind};
use city_math::Vec2;

use crate::builder::MeshBuilder;
use crate::palette::{self, facade_color};

/// Height of the kerb / block plateau in metres (the height `city-layout` walks up).
pub const KERB_HEIGHT: f32 = 0.15;

/// How far road paint floats above the asphalt, metres (z-fighting guard).
pub const MARKING_LIFT: f32 = 0.012;

/// Length of a painted centre-line dash, metres.
pub const DASH_LEN: f32 = 3.0;
/// Gap between two dashes, metres.
pub const DASH_GAP: f32 = 4.0;
/// Width of a painted line, metres.
pub const LINE_WIDTH: f32 = 0.16;

/// Build every **static** mesh of the city into `m`: the base plane, one raised surface
/// plus kerb band per block, every building volume (setbacks included) and the street
/// furniture. Road paint is excluded — see [`build_road_markings`].
pub fn build_city(city: &City, m: &mut MeshBuilder) {
    let b = city.bounds();

    // base plane: everything is tarmac, then blocks paint their own surface
    m.ground(
        [b.min.x, b.min.y],
        [b.max.x, b.max.y],
        0.0,
        palette::ASPHALT,
    );

    for block in city.blocks() {
        let col = block_surface_color(block.kind);
        let l = block.lots;
        // sidewalks / block surface, raised so blocks read as blocks
        m.ground([l.min.x, l.min.y], [l.max.x, l.max.y], KERB_HEIGHT, col);
        // kerb band around the block
        m.box_shaded(
            [block.bounds.min.x, 0.0, block.bounds.min.y],
            [block.bounds.max.x, KERB_HEIGHT, block.bounds.max.y],
            col,
            palette::CONCRETE,
        );
    }

    for building in city.buildings() {
        building_mesh(m, building);
    }

    for p in city.props() {
        prop(m, p);
    }
}

/// Paving colour of a block's surface by land use.
#[inline]
pub fn block_surface_color(kind: city_layout::BlockKind) -> [f32; 3] {
    match kind {
        city_layout::BlockKind::Park => palette::PARK,
        city_layout::BlockKind::Plaza => palette::PLAZA,
        city_layout::BlockKind::Lot => palette::LOT,
        city_layout::BlockKind::Urban => palette::SIDEWALK,
    }
}

/// One building: the main volume plus the inset setback tower when it has one.
pub fn building_mesh(m: &mut MeshBuilder, b: &city_layout::Building) {
    let f = &b.footprint;
    let wall = facade_color(b);
    m.box_shaded(
        [f.min.x, 0.0, f.min.y],
        [f.max.x, b.height, f.max.y],
        palette::ROOF,
        wall,
    );
    if b.setback_height > 0.2 {
        let size = f.size();
        let inset_x = size.x * (1.0 - b.setback_scale) * 0.5;
        let inset_z = size.y * (1.0 - b.setback_scale) * 0.5;
        m.box_shaded(
            [f.min.x + inset_x, b.height, f.min.y + inset_z],
            [
                f.max.x - inset_x,
                b.height + b.setback_height,
                f.max.y - inset_z,
            ],
            palette::ROOF,
            wall,
        );
    }
}

/// Street furniture of one prop: trees, lamps, benches, bins, hydrants, bus stops,
/// barriers, planters, bollards and monuments.
pub fn prop(m: &mut MeshBuilder, p: &city_layout::Prop) {
    let (x, z) = (p.pos.x, p.pos.y);
    let s = p.scale;
    match p.kind {
        PropKind::Tree => {
            let h = 2.6 * s;
            m.box_shaded(
                [x - 0.13, 0.0, z - 0.13],
                [x + 0.13, h, z + 0.13],
                palette::TRUNK,
                palette::TRUNK,
            );
            let cr = 1.3 * s;
            m.box_shaded(
                [x - cr, h, z - cr],
                [x + cr, h + 1.9 * s, z + cr],
                palette::LEAF,
                palette::shade(palette::LEAF, 0.82),
            );
        }
        PropKind::Lamp => {
            let h = 5.4 * s;
            m.box_shaded(
                [x - 0.10, 0.0, z - 0.10],
                [x + 0.10, h, z + 0.10],
                palette::METAL,
                palette::METAL,
            );
            // the head reads emissive at night (`Prop::glow`)
            let head = if p.glow > 0.05 {
                palette::LAMP_ON
            } else {
                palette::METAL
            };
            m.box_shaded(
                [x - 0.34, h, z - 0.17],
                [x + 0.34, h + 0.24, z + 0.16],
                head,
                head,
            );
        }
        PropKind::Bench => {
            let yaw = p.yaw;
            // seat slab, backrest and two metal legs
            m.box_yaw(
                [x, z],
                0.95,
                0.22,
                0.42,
                0.50,
                yaw,
                palette::BENCH_WOOD,
                palette::BENCH_WOOD,
            );
            m.box_yaw(
                [x, z],
                0.95,
                0.05,
                0.50,
                0.94,
                yaw,
                palette::BENCH_WOOD,
                palette::shade(palette::BENCH_WOOD, 0.85),
            );
            m.box_yaw(
                [x, z],
                0.90,
                0.18,
                0.0,
                0.42,
                yaw,
                palette::METAL,
                palette::METAL,
            );
        }
        PropKind::Bin => {
            let h = 0.95 * s;
            m.box_yaw(
                [x, z],
                0.32,
                0.32,
                0.0,
                h,
                p.yaw,
                palette::BIN_GREEN,
                palette::shade(palette::BIN_GREEN, 0.85),
            );
            m.box_yaw(
                [x, z],
                0.36,
                0.36,
                h,
                h + 0.10,
                p.yaw,
                palette::shade(palette::BIN_GREEN, 0.7),
                palette::METAL,
            );
        }
        PropKind::Hydrant => {
            let h = 0.75 * s;
            m.box_shaded(
                [x - 0.17, 0.0, z - 0.17],
                [x + 0.17, h, z + 0.17],
                palette::HYDRANT,
                palette::shade(palette::HYDRANT, 0.85),
            );
            m.box_shaded(
                [x - 0.27, h * 0.55, z - 0.10],
                [x + 0.27, h * 0.80, z + 0.10],
                palette::HYDRANT,
                palette::HYDRANT,
            );
        }
        PropKind::BusStop => {
            m.box_shaded(
                [x - 1.3, 0.0, z - 0.5],
                [x + 1.0, 2.4, z + 0.5],
                [0.52, 0.48, 0.52],
                [0.44, 0.42, 0.46],
            );
        }
        PropKind::Monument => {
            let h = 6.0 * s;
            m.box_shaded(
                [x - 0.9, 0.0, z - 0.9],
                [x + 0.8, h, z + 0.9],
                palette::MONUMENT,
                [0.52, 0.46, 0.58],
            );
        }
        PropKind::Barrier | PropKind::Planter | PropKind::Bollard => {
            let w = if p.kind == PropKind::Bollard {
                0.16
            } else {
                0.6
            };
            let h = if p.kind == PropKind::Bollard {
                0.85
            } else {
                1.1
            };
            m.box_shaded(
                [x - w, 0.0, z - w],
                [x + w, h, z + w],
                palette::CONCRETE,
                palette::CONCRETE,
            );
        }
    }
}

/// Paint the road markings: dashed centre lines along every carriageway and the zebra
/// bars of every marked crossing. Everything sits [`MARKING_LIFT`] above the asphalt.
pub fn build_road_markings(city: &City, m: &mut MeshBuilder) {
    for road in city.roads() {
        let seg = road.center_line(city.params());
        let len = seg.len();
        if len < DASH_LEN + 1.0 {
            continue;
        }
        let dir = seg.dir();
        let col = match road.kind {
            RoadKind::Avenue => palette::PAINT_YELLOW,
            RoadKind::Street => palette::PAINT_WHITE,
        };
        // keep the dashes inside the carriageway: start after the junction box
        let mut s = MARKING_INSET;
        while s + DASH_LEN <= len - MARKING_INSET {
            flat_quad(m, seg.a + dir * s, dir, DASH_LEN, LINE_WIDTH, col);
            s += DASH_LEN + DASH_GAP;
        }
    }

    for crossing in city.crossings() {
        zebra_crossing(m, crossing);
    }
}

/// Paint the zebra of one crossing: [`zebra_bars`] bars of [`ZEBRA_WIDTH`] laid out
/// across the walking band, each running kerb-to-kerb minus [`ZEBRA_MARGIN`].
pub fn zebra_crossing(m: &mut MeshBuilder, crossing: &city_layout::Crossing) {
    let dir = crossing.dir; // across the carriageway = along the walking path
    let along = dir.perp(); // the direction the stripes are stacked along
    let bars = zebra_bars(crossing.width);
    let span = crossing.width - 2.0 * ZEBRA_MARGIN;
    let run = crossing.length - 2.0 * ZEBRA_MARGIN;
    for i in 0..bars {
        let t = (i as f32 + 0.5) / bars as f32;
        let centre = crossing.center + along * ((t * 2.0 - 1.0) * span * 0.5);
        flat_quad(
            m,
            centre - dir * (run * 0.5),
            dir,
            run,
            ZEBRA_WIDTH,
            palette::PAINT_WHITE,
        );
    }
}

/// Number of zebra bars a crossing gets (at least two, at [`ZEBRA_PITCH`] pitch).
#[inline]
pub fn zebra_bars(band_width: f32) -> usize {
    ((band_width / ZEBRA_PITCH) as usize).clamp(2, 14)
}

/// Pitch between zebra bars, metres.
pub const ZEBRA_PITCH: f32 = 0.9;
/// Width of one zebra bar, metres.
pub const ZEBRA_WIDTH: f32 = 0.45;
/// Bare margin kept between the zebra bars and the kerbs, metres.
pub const ZEBRA_MARGIN: f32 = 0.5;
/// Keep markings this far away from the junction centres at both ends of a road.
pub const MARKING_INSET: f32 = 6.0;

/// A flat painted quad: starts at `from` (XZ), runs `length` along `dir`, `width` wide.
fn flat_quad(m: &mut MeshBuilder, from: Vec2, dir: Vec2, length: f32, width: f32, col: [f32; 3]) {
    let n = dir.perp() * (width * 0.5);
    let end = from + dir * length;
    let y = MARKING_LIFT;
    m.quad(
        [from.x - n.x, y, from.y - n.y],
        [from.x + n.x, y, from.y + n.y],
        [end.x - n.x, y, end.y - n.y],
        [end.x + n.x, y, end.y + n.y],
        [0.0, 1.0, 0.0],
        col,
    );
}

/// A flat painted quad centred at `center` (used by lot stripes and plazas).
pub fn flat_quad_centered(
    m: &mut MeshBuilder,
    center: Vec2,
    dir: Vec2,
    length: f32,
    width: f32,
    col: [f32; 3],
) {
    flat_quad(m, center - dir * (length * 0.5), dir, length, width, col);
}

/// Parking stripes of a surface lot: two rows of bays along the lot's long axis.
pub fn build_parking_stripes(city: &City, m: &mut MeshBuilder) {
    use city_layout::BlockKind;
    for block in city.blocks() {
        if block.kind != BlockKind::Lot {
            continue;
        }
        let l = block.lots;
        let size = l.size();
        let (long, long_len) = if size.x >= size.y {
            (Vec2::X, size.x)
        } else {
            (Vec2::Y, size.y)
        };
        let short = long.perp();
        let bays = ((long_len / PARKING_BAY_PITCH) as usize).saturating_sub(1);
        for i in 0..bays {
            let t = (i as f32 + 0.5) / bays as f32;
            let base = l.min + long * (long_len * t);
            for row in 0..2 {
                let side = if row == 0 { 1.0 } else { -1.0 };
                let c = base + short * (side * (PARKING_ISLE_HALF + PARKING_DEPTH * 0.5));
                flat_quad_centered(
                    m,
                    c,
                    long,
                    PARKING_LINE_WIDTH,
                    PARKING_DEPTH,
                    palette::PARKING_STRIPE,
                );
            }
        }
    }
}

/// Pitch between two parking-bay lines, metres.
pub const PARKING_BAY_PITCH: f32 = 2.6;
/// Depth of one parking bay, metres.
pub const PARKING_DEPTH: f32 = 5.0;
/// Half-width of the driving aisle between the two rows of bays, metres.
pub const PARKING_ISLE_HALF: f32 = 3.0;
/// Paint width of a bay line, metres.
pub const PARKING_LINE_WIDTH: f32 = 0.14;
