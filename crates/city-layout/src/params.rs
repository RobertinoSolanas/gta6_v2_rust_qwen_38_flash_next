//! Generation parameters — the "zoning code" of the city.

use city_math::{Aabb2, Vec2};

/// Relative share of the non-urban land uses (probabilities per block).
#[derive(Clone, Debug)]
pub struct LandMix {
    /// Chance that a block becomes a park.
    pub park: f32,
    /// Chance of a paved plaza.
    pub plaza: f32,
    /// Chance of a surface car park.
    pub lot: f32,
}

impl Default for LandMix {
    fn default() -> Self {
        LandMix {
            park: 0.11,
            plaza: 0.05,
            lot: 0.07,
        }
    }
}

/// Everything the generator needs to know.
///
/// Distances are metres; the player is ~1.8 m tall and a lane is 3.6 m wide.
#[derive(Clone, Debug)]
pub struct CityParams {
    /// World seed — same seed, same city.
    pub seed: u64,
    /// Blocks across (X).
    pub blocks_x: usize,
    /// Blocks along (Z).
    pub blocks_z: usize,
    /// Footprint of one block including its sidewalk band.
    pub block_size: f32,
    /// Carriageway width (two lanes).
    pub road_width: f32,
    /// Sidewalk band width on each block edge.
    pub sidewalk_width: f32,
    /// Minimum gap between a building wall and the inner edge of the sidewalk.
    pub lot_inset: f32,
    /// Width of the alley left between neighbouring buildings.
    pub alley_width: f32,
    /// Land use mix.
    pub land: LandMix,
    /// Typical height of a low-rise building (m).
    pub height_low: f32,
    /// Tallest ordinary building (m).
    pub height_max: f32,
    /// Extra height for the landmark towers near the centre.
    pub landmark_extra: f32,
    /// Average spacing of street trees (m).
    pub tree_spacing: f32,
    /// Average spacing of street lamps (m).
    pub lamp_spacing: f32,
    /// Street furniture per 100 m of sidewalk.
    pub furniture_density: f32,
    /// Every `major_period`-th street line is an avenue.
    pub major_period: usize,
}

impl Default for CityParams {
    fn default() -> Self {
        CityParams {
            seed: 0x50e0_17,
            blocks_x: 9,
            blocks_z: 9,
            block_size: 44.0,
            road_width: 9.2,
            sidewalk_width: 3.4,
            lot_inset: 1.0,
            alley_width: 2.6,
            land: LandMix::default(),
            height_low: 7.5,
            height_max: 32.0,
            landmark_extra: 48.0,
            tree_spacing: 7.5,
            lamp_spacing: 15.0,
            furniture_density: 5.0,
            major_period: 3,
        }
    }
}

impl CityParams {
    /// Distance between two consecutive block origins (block + one road).
    #[inline]
    pub fn pitch(&self) -> f32 {
        self.block_size + self.road_width
    }

    /// Total length of the grid on one axis.
    #[inline]
    pub fn extent(&self, n: usize) -> f32 {
        n.max(1) as f32 * self.block_size + (n + 1) as f32 * self.road_width
    }

    /// Full bounds of the generated city.
    #[inline]
    pub fn city_bounds(&self) -> Aabb2 {
        Aabb2::from_min_size(
            Vec2::ZERO,
            Vec2::new(self.extent(self.blocks_x), self.extent(self.blocks_z)),
        )
    }

    /// Grid size in blocks.
    #[inline]
    pub fn grid(&self) -> (usize, usize) {
        (self.blocks_x.max(1), self.blocks_z.max(1))
    }

    /// Centre of road line `i` (there are `blocks + 1` lines per axis).
    #[inline]
    pub fn road_center(&self, i: usize) -> f32 {
        i as f32 * self.pitch() + self.road_width * 0.5
    }

    /// Minimum coordinate of block column `i`.
    #[inline]
    pub fn block_min(&self, i: usize) -> f32 {
        self.road_width + i as f32 * self.pitch()
    }

    /// Road lines perpendicular to X.
    #[inline]
    pub fn road_lines_x(&self) -> usize {
        self.blocks_x + 1
    }

    /// Road lines perpendicular to Z.
    #[inline]
    pub fn road_lines_z(&self) -> usize {
        self.blocks_z + 1
    }

    /// Total number of junction nodes.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.road_lines_x() * self.road_lines_z()
    }

    /// Clamp a point into the city, keeping it clear of the outer wall.
    pub fn clamp_to_city(&self, p: Vec2) -> Vec2 {
        let b = self.city_bounds();
        let m = self.road_width * 0.5 - 0.5;
        Vec2::new(
            city_math::clamp(p.x, b.min.x + m, b.max.x - m),
            city_math::clamp(p.y, b.min.y + m, b.max.y - m),
        )
    }

    /// Distance from `p` to the city centre normalised to `0..=1`
    /// (`0` = downtown core, `1` = outer ring).
    pub fn centrality(&self, p: Vec2) -> f32 {
        let b = self.city_bounds();
        let c = b.center();
        let half = Vec2::new(b.size().x, b.size().y).len() * 0.5;
        city_math::saturate(p.dist(c) / half.max(1.0))
    }
}
