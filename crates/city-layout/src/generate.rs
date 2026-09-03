//! City assembly: turns [`CityParams`] into blocks, buildings, props and indexes.
//!
//! The order of operations is fixed (so a seed always yields the same city):
//! 1. roads & lanes → 2. intersections + lane graph → 3. blocks → 4. buildings →
//! 5. crossings → 6. sidewalk loops + links → 7. props → 8. collision index →
//! 9. a guaranteed walkable spawn point.

use city_math::{Aabb2, Rng, Vec2, TAU};

use crate::buildings::{self, Building};
use crate::index::{IndexItem, IndexKind, SpatialIndex};
use crate::params::CityParams;
use crate::props::{Prop, PropKind};
use crate::walk::{self, SidewalkLoop};
use crate::{Block, BlockKind};

/// Deterministic per-block RNG seed.
#[inline]
pub fn block_seed(seed: u64, cell: [usize; 2], salt: u64) -> u64 {
    city_math::hash12(city_math::hash12(seed, salt), cell[0] as u64 * 738_581 + cell[1] as u64)
}

/// Choose the land use of a block.
pub fn pick_block_kind(cell: [usize; 2], params: &CityParams, rng: &mut Rng) -> BlockKind {
    let (ix, iz) = cell;
    let r = rng.next_f32();
    let central = ix.abs_diff(params.blocks_x / 2) <= 1 && iz.abs_diff(params.blocks_z / 2) <= 1;
    // The core keeps its towers: green space is rare downtown.
    let park = params.land.park * if central { 0.35 } else { 1.0 };
    if r < park {
        BlockKind::Park
    } else if r < park + params.land.plaza {
        BlockKind::Plaza
    } else if r < park + params.land.plaza + params.land.lot {
        BlockKind::Lot
    } else {
        BlockKind::Urban
    }
}

/// Bounds of block `(ix, iz)`.
#[inline]
pub fn block_bounds(params: &CityParams, ix: usize, iz: usize) -> Aabb2 {
    Aabb2::from_min_size(
        Vec2::new(params.block_min(ix), params.block_min(iz)),
        Vec2::new(params.block_size, params.block_size),
    )
}

/// Buildable lot area of a block (inside the sidewalk band).
#[inline]
pub fn lot_area(bounds: Aabb2, params: &CityParams) -> Aabb2 {
    bounds.grown(-(params.sidewalk_width + params.lot_inset))
}

/// Build all blocks (without their contents yet).
pub fn build_blocks(params: &CityParams, blocks: &mut Vec<Block>) {
    for ix in 0..params.blocks_x {
        for iz in 0..params.blocks_z {
            let cell = [ix, iz];
            let mut rng = Rng::new(block_seed(params.seed, cell, 0xb10c5));
            let bounds = block_bounds(params, ix, iz);
            let kind = pick_block_kind(cell, params, &mut rng);
            let edge =
                ix == 0 || iz == 0 || ix + 1 == params.blocks_x || iz + 1 == params.blocks_z;
            blocks.push(Block {
                cell,
                bounds,
                lots: lot_area(bounds, params),
                kind,
                buildings: Vec::new(),
                props: Vec::new(),
                loop_index: ix * params.blocks_z + iz,
                edge,
            });
        }
    }
}

/// Add the buildings of every block; records the new ids on each block.
pub fn fill_buildings(params: &CityParams, blocks: &mut [Block], out: &mut Vec<Building>) {
    for idx in 0..blocks.len() {
        let cell = blocks[idx].cell;
        let kind = blocks[idx].kind;
        let lots = blocks[idx].lots;
        let mut rng = Rng::new(block_seed(params.seed, cell, 0xb0c));
        let before = out.len();
        buildings::build_block_buildings(idx, kind, lots, params, &mut rng, out);
        blocks[idx].buildings = (before..out.len()).collect();
    }
}

/// Place the props of every block along its sidewalk loop.
pub fn fill_props(
    params: &CityParams,
    blocks: &mut [Block],
    loops: &[SidewalkLoop],
    out: &mut Vec<Prop>,
) {
    for idx in 0..blocks.len() {
        let cell = blocks[idx].cell;
        let kind = blocks[idx].kind;
        let Some(loop_) = loops.get(blocks[idx].loop_index) else {
            continue;
        };
        let mut rng = Rng::new(block_seed(params.seed, cell, 0xf05));
        let ids = place_block_props(idx, kind, loop_, params, &mut rng, out);
        blocks[idx].props = ids;
    }
}

/// Place one block's props. Returns the ids created.
pub fn place_block_props(
    block: usize,
    kind: BlockKind,
    loop_: &SidewalkLoop,
    params: &CityParams,
    rng: &mut Rng,
    out: &mut Vec<Prop>,
) -> Vec<usize> {
    let mut ids = Vec::new();
    let perim = loop_.perimeter();
    if perim < 6.0 {
        return ids;
    }
    let kerb = params.sidewalk_width * 0.30;

    // Trees along the kerb.
    let mut s = rng.range_f32(0.0, params.tree_spacing.max(2.0));
    while s < perim {
        s += params.tree_spacing.max(2.0) * rng.range_f32(0.85, 1.3);
        let id = out.len();
        out.push(Prop {
            id,
            kind: PropKind::Tree,
            pos: kerb_point(loop_, s, kerb),
            yaw: rng.next_f32() * TAU,
            scale: rng.range_f32(0.85, 1.35),
            block,
            casts_shadow: true,
            glow: 0.0,
        });
        ids.push(id);
    }

    // Street lamps on a wider rhythm.
    let mut s = rng.range_f32(0.0, params.lamp_spacing.max(4.0));
    while s < perim {
        s += params.lamp_spacing.max(4.0) * rng.range_f32(0.9, 1.2);
        let id = out.len();
        out.push(Prop {
            id,
            kind: PropKind::Lamp,
            pos: kerb_point(loop_, s, kerb * 0.6),
            yaw: loop_.dir_at(s).angle(),
            scale: 1.0,
            block,
            casts_shadow: true,
            glow: 1.0,
        });
        ids.push(id);
    }

    // Sparse furniture.
    let count = ((perim / 100.0) * params.furniture_density.max(0.0)).round() as i32;
    for _ in 0..count {
        let s = rng.next_f32() * perim;
        let pick = pick_furniture(kind, rng);
        let id = out.len();
        out.push(Prop {
            id,
            kind: pick,
            pos: kerb_point(loop_, s, kerb + 0.35),
            yaw: loop_.dir_at(s).angle(),
            scale: rng.range_f32(0.9, 1.15),
            block,
            casts_shadow: pick.height() > 1.2,
            glow: if pick == PropKind::Monument { 1.0 } else { 0.0 },
        });
        ids.push(id);
    }

    // Extra trees scattered inside parks.
    if kind == BlockKind::Park {
        let extras = ((perim * 0.35) / params.tree_spacing.max(2.0)) as i32;
        for _ in 0..extras {
            let id = out.len();
            out.push(Prop {
                id,
                kind: PropKind::Tree,
                pos: park_interior(loop_, rng),
                yaw: rng.next_f32() * TAU,
                scale: rng.range_f32(0.9, 1.55),
                block,
                casts_shadow: true,
                glow: 0.0,
            });
            ids.push(id);
        }
    }

    ids
}

/// Land-use centrepieces: plaza pylon + planters, park benches.
pub fn place_centrepieces(params: &CityParams, blocks: &[Block], out: &mut Vec<Prop>) {
    for (idx, b) in blocks.iter().enumerate() {
        let c = b.bounds.center();
        match b.kind {
            BlockKind::Plaza => {
                let mut rng = Rng::new(block_seed(params.seed, b.cell, 0xace5));
                let id = out.len();
                out.push(Prop {
                    id,
                    kind: PropKind::Monument,
                    pos: c,
                    yaw: rng.next_f32() * TAU,
                    scale: rng.range_f32(0.8, 1.25),
                    block: idx,
                    casts_shadow: true,
                    glow: 1.0,
                });
                for i in 0..4 {
                    let a = i as f32 * (TAU / 4.0) + core::f32::consts::FRAC_PI_4;
                    let p = c + Vec2::new(a.cos(), a.sin()) * b.bounds.size().x * 0.3;
                    let id = out.len();
                    out.push(Prop {
                        id,
                        kind: PropKind::Planter,
                        pos: p,
                        yaw: a,
                        scale: 1.0,
                        block: idx,
                        casts_shadow: false,
                        glow: 0.0,
                    });
                }
            }
            BlockKind::Park => {
                for i in 0..3 {
                    let a = i as f32 * (TAU / 3.0);
                    let id = out.len();
                    out.push(Prop {
                        id,
                        kind: PropKind::Bench,
                        pos: c + Vec2::new(a.cos(), a.sin()) * 6.5,
                        yaw: a + core::f32::consts::FRAC_PI_2,
                        scale: 1.0,
                        block: idx,
                        casts_shadow: true,
                        glow: 0.0,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Insert every solid object into the collision index.
pub fn build_index(
    buildings: &[Building],
    props: &[Prop],
    index: &mut SpatialIndex,
) {
    for b in buildings {
        index.insert(IndexItem {
            id: b.id,
            kind: IndexKind::Building,
            solid: b.footprint,
            height: b.top(),
        });
    }
    for p in props {
        if !p.blocks_walk() {
            continue;
        }
        index.insert(IndexItem {
            id: p.id,
            kind: IndexKind::Prop,
            solid: p.world_footprint(),
            height: p.top(),
        });
    }
}

/// A walkable spawn point close to the city centre.
pub fn find_spawn(index: &SpatialIndex, loops: &[SidewalkLoop], centre: Vec2) -> Vec2 {
    let mut best: Option<(f32, Vec2)> = None;
    for loop_ in loops {
        for &p in loop_.points() {
            if index.overlaps_circle(p, 0.7) {
                continue;
            }
            let d = centre.dist(p);
            if best_is_better(d, best.map(|(bd, _)| bd)) {
                best = Some((d, p));
            }
        }
    }
    best.map(|(_, p)| p).unwrap_or(centre)
}

/// Point on the loop pushed `off` metres towards the kerb (away from the block).
fn kerb_point(loop_: &SidewalkLoop, s: f32, off: f32) -> Vec2 {
    let p = loop_.point_at(s);
    // The loop surrounds its block, so "away from the loop centre" is outwards.
    let centre = loop_centre(loop_);
    let dir = (p - centre).norm();
    let d = if d.len_sq() < 1e-6 {
        outward_from_loop(loop_, p, 0.0)
    } else {
        d
    };
    p + d * off
}

/// Average centre of a loop.
fn loop_centre(loop_: &SidewalkLoop) -> Vec2 {
    if loop_.points().is_empty() {
        return Vec2::ZERO;
    }
    let mut acc = Vec2::ZERO;
    for p in loop_.points() {
        acc_add(&mut acc, *p);
    }
    let _ = acc;
    acc * (1.0 / loop_.points().len() as f32)
}

/// A point inside a park block (between the loop and its centre).
fn park_interior(loop_: &SidewalkLoop, rng: &mut Rng) -> Vec2 {
    let p = loop_.point_at(rng.next_f32() * loop_.perimeter());
    let centre = loop_centre(loop_);
    let t = rng.range_f32(0.15, 0.9);
    p.lerp(centre, t)
}

/// Sidewalk prop selection: parks get benches/bins, urban blocks the full mix.
pub fn pick_furniture(block: BlockKind, rng: &mut Rng) -> PropKind {
    let weights = match block {
        BlockKind::Park => [0.0f32, 0.0, 6.0, 3.0, 0.2, 0.0, 0.0, 2.0, 0.0, 0.0],
        BlockKind::Plaza => [0.0, 0.0, 4.0, 2.0, 0.3, 0.0, 0.0, 3.0, 0.0, 1.0],
        BlockKind::Lot => [0.0, 1.0, 0.5, 1.5, 0.2, 0.0, 2.0, 0.0, 0.0, 2.0],
        BlockKind::Urban => [0.0, 0.0, 2.0, 3.0, 1.0, 1.2, 0.6, 0.8, 0.1, 0.8],
    };
    let idx = rng.weighted(&weights);
    PropKind::ALL[idx]
}

fn acc_add(a: &mut Vec2, b: Vec2) {
    *a = *a + b;
}

fn find_spawn(params: &CityParams, index: &SpatialIndex, loops: &[SidewalkLoop]) -> Vec2 {
    let centre = params.city_bounds().center();
    let mut best: Option<(f32, Vec2)> = None;
    for loop_ in loops {
        for &p in loop_.points() {
            if index.overlaps_circle(p, 0.7) {
                continue;
            }
            let d = centre.dist(p);
            if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, p));
            }
        }
    }
    best.map(|(_, p)| p).unwrap_or(centre)
}
