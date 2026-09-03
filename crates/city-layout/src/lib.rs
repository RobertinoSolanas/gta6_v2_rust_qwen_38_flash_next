//! # city-layout — the city generator (bounded context: *urban form*)
//!
//! Speaks the language of blocks, roads, sidewalks, lots, facades and street props.
//! Everything derives from [`CityParams`] plus a seed: the same parameters always
//! produce the exact same city, which is what makes the native *and* the browser
//! runtime tests comparable.
//!
//! ## Model
//!
//! * A regular grid of [`Block`]s separated by [`Road`]s (carriageways) with a
//!   [`Lane`] in each direction.
//! * Every block carries a sidewalk band; the remaining lot area is subdivided into
//!   [`Building`]s (with alleys between them). Some blocks are parks, plazas or
//!   surface car parks.
//! * [`Prop`]s (trees, lamps, benches, bins, hydrants, bus stops, barriers) line the
//!   sidewalks.
//! * The pedestrian network is one [`SidewalkLoop`] per block joined by
//!   [`CrossingLink`]s at the intersections — that is what makes the crowd look like
//!   it belongs to the street.
//! * Cars drive on [`Lane`]s and pick a new lane at each [`Intersection`].
//!
//! ```
//! use city_layout::CityParams;
//! use city_layout::City;
//!
//! let city = City::generate(CityParams::default());
//! assert!(city.buildings().len() > 50);
//! assert!(city.is_walkable(city.spawn_point()));
//! ```

#![forbid(unsafe_code)]

mod buildings;
mod index;
mod params;
mod props;
mod roads;
mod walk;

pub use buildings::{FacadeStyle, Building, RoofKind};
pub use index::{SpatialIndex, CELL_SIZE};
pub use params::{CityParams, LandMix};
pub use props::{Prop, PropKind};
pub use roads::{Axis, Crossing, Intersection, Lane, Road, RoadKind};
pub use walk::{CrossingLink, SidewalkLoop};

use city_math::{Aabb2, Rng, Vec2};

/// Land use of a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    /// Built-up block (tower / mid-rise / row houses).
    Urban,
    /// Green block: lawn, trees, benches, no buildings.
    Park,
    /// Open paved plaza with a monument and benches.
    Plaza,
    /// Surface car park with stripes and parked cars.
    Lot,
}

/// A city block: bounds, land use, and the ids of everything inside it.
#[derive(Clone, Debug)]
pub struct Block {
    /// Grid coordinate (`ix` across X, `iz` along Z).
    pub cell: [usize; 2],
    /// Full block bounds, including the sidewalk band.
    pub bounds: Aabb2,
    /// Lot area: `bounds` inset by the sidewalk width.
    pub lots: Aabb2,
    /// Land use.
    pub kind: BlockKind,
    /// Ids into [`City::buildings`].
    pub buildings: Vec<usize>,
    /// Ids of the [`Prop`]s on this block's sidewalks / interior.
    pub props: Vec<usize>,
    /// Index into [`City::loops`].
    pub loop_index: usize,
    /// `true` when the block touches the city border.
    pub edge: bool,
}

/// The generated city: every static piece of the world.
#[derive(Clone, Debug)]
pub struct City {
    params: CityParams,
    blocks: Vec<Block>,
    buildings: Vec<Building>,
    props: Vec<Prop>,
    roads: Vec<Road>,
    lanes: Vec<Lane>,
    intersections: Vec<Intersection>,
    crossings: Vec<Crossing>,
    loops: Vec<SidewalkLoop>,
    links: Vec<CrossingLink>,
    index: SpatialIndex,
    bounds: Aabb2,
    spawn: Vec2,
}

impl City {
    /// Generate a whole city. Runs in a few milliseconds for the default parameters.
    pub fn generate(params: CityParams) -> City {
        let bounds = params.city_bounds();
        let mut city = City {
            params,
            bounds,
            blocks: Vec::new(),
            buildings: Vec::new(),
            props: Vec::new(),
            roads: Vec::new(),
            lanes: Vec::new(),
            intersections: Vec::new(),
            crossings: Vec::new(),
            loops: Vec::new(),
            links: Vec::new(),
            index: SpatialIndex::new(CELL_SIZE),
            spawn: Vec2::ZERO,
        };
        city.build_roads();
        city.build_intersections();
        city.build_blocks(&mut Rng::new(city.params.seed));
        city.build_walk_network();
        city.build_props(&mut Rng::new(city.params.seed ^ 0x9e37_79b9));
        city.rebuild_index();
        city.pick_spawn();
        city
    }

    // --- accessors -------------------------------------------------------

    /// Parameters this city was generated from.
    #[inline]
    pub fn params(&self) -> &CityParams {
        &self.params
    }
    /// All blocks (row-major over the grid).
    #[inline]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
    /// All buildings.
    #[inline]
    pub fn buildings(&self) -> &[Building] {
        &self.buildings
    }
    /// One building by id.
    #[inline]
    pub fn building(&self, id: usize) -> Option<&Building> {
        self.buildings.get(id)
    }
    /// All street props.
    #[inline]
    pub fn props(&self) -> &[Prop] {
        &self.props
    }
    /// Carriageways.
    #[inline]
    pub fn roads(&self) -> &[Road] {
        &self.roads
    }
    /// Lanes (one per direction per road).
    #[inline]
    pub fn lanes(&self) -> &[Lane] {
        &self.lanes
    }
    /// Intersections (grid nodes).
    #[inline]
    pub fn intersections(&self) -> &[Intersection] {
        &self.intersections
    }
    /// Marked pedestrian crossings.
    #[inline]
    pub fn crossings(&self) -> &[Crossing] {
        &self.crossings
    }
    /// Sidewalk loops.
    #[inline]
    pub fn loops(&self) -> &[SidewalkLoop] {
        &self.loops
    }
    /// Links between loops across a road.
    #[inline]
    pub fn links(&self) -> &[CrossingLink] {
        &self.links
    }
    /// Outer city bounds (including the bordering roads).
    #[inline]
    pub fn bounds(&self) -> Aabb2 {
        self.bounds
    }
    /// Guaranteed walkable spawn point.
    #[inline]
    pub fn spawn_point(&self) -> Vec2 {
        self.spawn
    }
    /// Broad-phase index of solid obstacles.
    #[inline]
    pub fn index(&self) -> &SpatialIndex {
        &self.index
    }
    /// Grid size in blocks.
    #[inline]
    pub fn grid(&self) -> [usize; 2] {
        (self.params.grid()).into()
    }

    // --- queries ---------------------------------------------------------

    /// Block at a grid coordinate.
    #[inline]
    pub fn block_at_grid(&self, ix: usize, iz: usize) -> Option<&Block> {
        if ix >= self.params.blocks_x || iz >= self.params.blocks_z {
            return None;
        }
        Some(&self.blocks[ix * self.params.blocks_z + iz])
    }

    /// Block containing `p` (grid lookup, O(1)).
    pub fn block_at(&self, p: Vec2) -> Option<&Block> {
        let p = self.params.clamp_to_city(p);
        let pitch = self.params.pitch();
        let lo = self.bounds.min;
        let ix = ((p.x - lo.x) / pitch) as i32;
        let iz = ((p.y - lo.y) / pitch) as i32;
        self.block_at_grid(ix.max(0) as usize, iz.max(0) as usize)
    }

    /// `true` when a circle of `radius` at `p` is free of solid geometry and
    /// still inside the city.
    #[inline]
    pub fn is_walkable(&self, p: Vec2, radius: f32) -> bool {
        let inside = self.bounds.grown(-radius).contains(p);
        inside && !self.index.overlaps_circle(p, radius)
    }

    /// Slide a circle of `radius` at `p` out of solid geometry.
    ///
    /// Returns the corrected position and the total correction applied; when the
    /// result is still inside something the caller keeps the last valid position.
    pub fn resolve(&self, p: Vec2, radius: f32) -> Vec2 {
        let mut pos = self.params.clamp_to_city(p);
        // Two passes: pushing out of one box can push into the neighbour.
        for _ in 0..3 {
            let mut moved = Vec2::ZERO;
            for id in self.index.candidates(pos, radius + 0.6) {
                if let Some(item) = self.index.item(id) {
                    if let Some(fixed) = item.solid.push_out(pos, radius) {
                        pos = fixed;
                    }
                }
            }
            if moved.length() < 1e-5 {
                break;
            }
        }
        pos
    }

    /// Nearest intersection centre to `p` (used by the HUD and by traffic).
    pub fn nearest_intersection(&self, p: Vec2) -> Option<&Intersection> {
        let mut best: Option<(f32, &Intersection)> = None;
        for it in &self.intersections {
            let d = p.dist_sq(it.center);
            if best.map(|(bd, _)| best_is_closer(best, (it.center, it))) != Some(false) {
            }
            let _ = best;
            best = match best {
                Some((bd, node)) => {
                    if bd <= it2_d(bd, p) {
                        Some((bd, node))
                    } else {
                        Some((bd, node))
                    }
                }
                None => Some((d, it)),
            };
        }
        best.map(|(_, it)| it)
    }

    /// Distance to the nearest road centreline (0 when standing on tarmac).
    pub fn distance_to_road(&self, p: Vec2) -> f32 {
        let mut best = f32::MAX;
        for r in &self.roads {
            let d = (r.seg().distance(p) - r.half_width).max(0.0);
            if d < best {
                best = d;
            }
        }
        best
    }

    /// Ids of buildings whose footprint contains `p`.
    pub fn buildings_at(&self, p: Vec2) -> Vec<usize> {
        let mut out = Vec::new();
        for (i, b) in self.buildings.iter().enumerate() {
            if b.footprint.contains(p) {
                out.push(i);
            }
        }
        out
    }
}

fn it2_d(d: f32, _p: Vec2) -> f32 {
    d
}
fn best_cmp(a: f32, b: f32) -> bool {
    b < a
}
