//! Buildings: massing, facade style and roof furniture.
//!
//! A [`Building`] is a box plus the metadata the renderer needs to shade its
//! facade procedurally (window grid, banding, shopfront, neon accents). Placement
//! lives in [`crate::generate`].

use city_math::{Aabb2, Rng, Vec2};

use crate::params::CityParams;
use crate::BlockKind;

/// Facade vocabulary. Each value maps to a fragment-shader branch, so a new style
/// is code — never an image file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacadeStyle {
    /// Brick / brownstone with small regular windows.
    Brick,
    /// Rendered stucco with cornices.
    Stucco,
    /// Precast concrete panel grid.
    Panel,
    /// Glazed curtain wall.
    Glass,
    /// Banded painted concrete.
    Concrete,
}

impl FacadeStyle {
    /// All styles, in pick order.
    pub const ALL: [FacadeStyle; 5] = [
        FacadeStyle::Brick,
        FacadeStyle::Stucco,
        FacadeStyle::Panel,
        FacadeStyle::Glass,
        FacadeStyle::Concrete,
    ];

    /// Relative pick weights matching [`FacadeStyle::ALL`].
    pub const WEIGHTS: [f32; 5] = [3.0, 2.6, 2.0, 3.0, 1.4];

    /// Index of this style inside [`FacadeStyle::ALL`] (shader variant).
    #[inline]
    pub fn index(self) -> usize {
        match self {
            FacadeStyle::Brick => 0,
            FacadeStyle::Stucco => 1,
            FacadeStyle::Panel => 2,
            FacadeStyle::Glass => 3,
            FacadeStyle::Concrete => 4,
        }
    }

    /// Pick a style from an RNG.
    pub fn pick(rng: &mut city_math::Rng) -> FacadeStyle {
        FacadeStyle::ALL[rng.weighted(&FacadeStyle::WEIGHTS)]
    }

    /// `true` when the ground floor should be a lit shopfront.
    #[inline]
    pub fn wants_shopfront(self, floors: u8) -> bool {
        floors <= 4
            || matches!(
                self,
                FacadeStyle::Glass | FacadeStyle::Stucco | FacadeStyle::Panel
            )
    }

    /// Window width in metres.
    #[inline]
    pub fn window_width(self) -> f32 {
        match self {
            FacadeStyle::Brick => 1.15,
            FacadeStyle::Stucco => 1.3,
            FacadeStyle::Panel => 1.45,
            FacadeStyle::Glass => 2.05,
            FacadeStyle::Concrete => 1.5,
        }
    }

    /// Specular strength of the wall material.
    #[inline]
    pub fn specular(self) -> f32 {
        match self {
            FacadeStyle::Glass => 0.85,
            FacadeStyle::Panel => 0.18,
            FacadeStyle::Brick => 0.06,
            FacadeStyle::Stucco => 0.10,
            FacadeStyle::Concrete => 0.12,
        }
    }

    /// Base wall colour as linear RGB.
    pub fn base_colour(self, variant: u8) -> [f32; 3] {
        match self {
            FacadeStyle::Brick => match variant % 3 {
                0 => [0.40, 0.27, 0.23],
                1 => [0.52, 0.36, 0.25],
                _ => [0.60, 0.48, 0.40],
            },
            FacadeStyle::Stucco => match variant % 4 {
                0 => [0.80, 0.74, 0.64],
                1 => [0.72, 0.66, 0.58],
                2 => [0.87, 0.82, 0.71],
                _ => [0.64, 0.59, 0.54],
            },
            FacadeStyle::Panel => match variant % 3 {
                0 => [0.62, 0.60, 0.57],
                1 => [0.55, 0.55, 0.56],
                _ => [0.68, 0.66, 0.62],
            },
            FacadeStyle::Glass => match variant % 3 {
                0 => [0.22, 0.32, 0.58],
                1 => [0.28, 0.40, 0.66],
                _ => [0.18, 0.28, 0.42],
            },
            FacadeStyle::Concrete => [0.62, 0.62, 0.60],
        }
    }
}

/// Roof furniture type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoofKind {
    /// Flat gravel roof with a parapet.
    Flat,
    /// Flat roof ringed by mechanical plant.
    Mechanical,
    /// Stepped crown for tall towers.
    Crown,
    /// Low-rise roof with a water tank.
    WaterTank,
}

/// A building mass.
#[derive(Clone, Debug)]
pub struct Building {
    /// Id inside [`crate::City::buildings`].
    pub id: usize,
    /// Owning block id.
    pub block: usize,
    /// Footprint on the ground plane.
    pub footprint: Aabb2,
    /// Height of the main volume in metres.
    pub height: f32,
    /// Height of the upper setback volume (0 when there is none).
    pub setback_height: f32,
    /// Linear scale of the setback volume relative to the footprint (`0..=1`).
    pub setback_scale: f32,
    /// Facade style.
    pub style: FacadeStyle,
    /// Roof furniture.
    pub roof: RoofKind,
    /// Storey height in metres.
    pub floor_height: f32,
    /// Cached storey count.
    pub floors: u8,
    /// Window columns on the X-facing walls.
    pub windows_x: u8,
    /// Window columns on the Z-facing walls.
    pub windows_z: u8,
    /// `true` when the ground floor is a lit shopfront.
    pub shopfront: bool,
    /// Style variant (0..=7) fed to the shader.
    pub variant: u8,
    /// Per-building seed for procedural detail.
    pub seed: u64,
    /// Landmark tower: neon crown, brightest windows.
    pub landmark: bool,
    /// Accent colour for crown / signage.
    pub accent: [f32; 3],
}

impl Building {
    /// Neon palette used for crowns and signage.
    pub const NEON_PALETTE: [[f32; 3]; 6] = [
        [1.00, 0.18, 0.42],
        [0.20, 0.85, 1.00],
        [1.00, 0.62, 0.12],
        [0.55, 1.00, 0.45],
        [0.72, 0.35, 1.00],
        [1.00, 0.85, 0.25],
    ];

    /// Storeys derived from the height.
    #[inline]
    pub fn storeys(&self) -> f32 {
        self.height / self.floor_height.max(0.5)
    }

    /// Height including the setback volume.
    #[inline]
    pub fn top(&self) -> f32 {
        self.height + self.setback_height
    }

    /// Footprint centre.
    #[inline]
    pub fn center(&self) -> Vec2 {
        self.footprint.center()
    }

    /// Longer facade span in metres.
    #[inline]
    pub fn facade_span(&self) -> f32 {
        let s = self.footprint.size();
        s.x.max(s.y)
    }

    /// Total window cells over the four facades.
    pub fn window_cells(&self) -> usize {
        let storeys = self.storeys().floor().max(1.0) as usize;
        let cols = self.windows_x as usize + self.windows_z as usize;
        storeys * cols * 2
    }

    /// `true` when the building rises well above the average roofline.
    #[inline]
    pub fn is_tall(&self) -> bool {
        self.top() > 40.0
    }

    /// Chance that a given window is lit at night.
    pub fn lit_ratio(&self) -> f32 {
        let base = match self.style {
            FacadeStyle::Glass => 0.52,
            FacadeStyle::Panel => 0.38,
            FacadeStyle::Concrete => 0.30,
            FacadeStyle::Stucco => 0.30,
            FacadeStyle::Brick => 0.26,
        };
        if self.landmark {
            f32::min(base + 0.35, 0.95)
        } else {
            base
        }
    }

    /// Emissive colour of the windows at night.
    pub fn window_colour(&self) -> [f32; 3] {
        match self.variant % 4 {
            0 => [1.00, 0.86, 0.62],
            1 => [1.00, 0.94, 0.80],
            2 => [0.86, 0.92, 1.00],
            _ => [1.00, 0.78, 0.45],
        }
    }
}

// --- placement ------------------------------------------------------------
//
// Massing strategy: split the lot into bands along X, then each band into plots
// along Z. Every leaf cell takes one mass, and the alley width is kept as the gap,
// which is what produces the "alley" gaps between neighbours.

/// Metres of facade per window column.
const WINDOW_SPACING: f32 = 3.2;
/// A mass narrower than this on either axis is not worth building.
const MIN_PLOT: f32 = 7.5;
/// Setback volume height as a fraction of the main volume.
const SETBACK_RATIO: f32 = 0.28;

/// Window columns that fit into `span` metres.
#[inline]
fn window_columns(span: f32) -> u8 {
    ((span / WINDOW_SPACING).floor() as i32).clamp(1, u8::MAX as i32) as u8
}

/// Pick a roof type from the height of the main volume.
fn pick_roof(height: f32, rng: &mut Rng) -> RoofKind {
    if height > 45.0 {
        RoofKind::Crown
    } else if height > 18.0 {
        if rng.next_f32() < 0.55 {
            RoofKind::Mechanical
        } else {
            RoofKind::Flat
        }
    } else if rng.next_f32() < 0.45 {
        RoofKind::WaterTank
    } else {
        RoofKind::Flat
    }
}

/// Split `[lo, hi]` into `n` slices separated by `gap` (alleys).
fn slices(lo: f32, hi: f32, n: usize, gap: f32) -> Vec<(f32, f32)> {
    let span = hi - lo;
    if span <= gap {
        return Vec::new();
    }
    let n = n.max(1);
    let unit = (span - gap * (n - 1) as f32) / n as f32;
    if unit_too_small(unit) {
        return vec![(lo, hi)];
    }
    (0..n)
        .map(|i| {
            let a = lo + i as f32 * (unit + gap);
            let b = if i + 1 == n { hi } else { a + unit };
            (a, b)
        })
        .collect()
}

#[inline]
fn unit_too_small(unit: f32) -> bool {
    unit < MIN_PLOT
}

/// Build every building of one block into `out`.
///
/// Deterministic: the same `seed`, `lot` and `params` always yield the same masses.
pub fn build_block_buildings(
    block: usize,
    kind: BlockKind,
    lot: Aabb2,
    params: &CityParams,
    rng: &mut Rng,
    out: &mut Vec<Building>,
) {
    if kind != BlockKind::Urban {
        return;
    }
    let gap = params.alley_width.max(0.5);
    // Downtown gets the towers: height scales with distance from the core.
    let downtown = 1.0 - city_math::saturate(params.centrality(lot.center()));
    let bands = slices(
        lot.min.x,
        lot.max.x,
        2 + (rng.next_f32() * 2.0) as usize,
        gap,
    );
    for (x0, x1) in bands {
        let cols = slices(
            lot.min.y,
            lot.max.y,
            1 + (rng.next_f32() * 2.8) as usize,
            gap,
        );
        for (y0, y1) in cols {
            // Occasional courtyard: leave this plot empty as a light well / inner yard.
            if rng.next_f32() < 0.12 {
                continue;
            }
            let plot = Aabb2::new(Vec2::new(x0, y0), Vec2::new(x1, y1));
            if plot.size().x < MIN_PLOT || plot.size().y < MIN_PLOT {
                continue;
            }
            out.push(make_building(block, plot, params, rng, downtown));
        }
    }
}

/// Height of an ordinary mass at `p` (metres), before the downtown bonus.
fn target_height(params: &CityParams, rng: &mut Rng) -> f32 {
    params.height_low + (params.height_max - params.height_low) * rng.range_f32(0.05, 1.0)
}

/// Scale `height` by how central the plot is (`downtown` = 1 downtown, 0 at the rim).
///
/// The `powi(6)` curve keeps the skyline flat over most of the grid and spikes the
/// landmark towers in the core.
fn apply_downtown(params: &CityParams, height: f32, downtown: f32, rng: &mut Rng) -> f32 {
    height + params.landmark_extra * downtown.powi(6) * rng.range_f32(0.35, 1.0)
}

fn make_building(
    block: usize,
    plot: Aabb2,
    params: &CityParams,
    rng: &mut Rng,
    downtown: f32,
) -> Building {
    let style = FacadeStyle::pick(rng);
    let height = target_height(params, rng);
    // Tall towers get a setback volume so the skyline is not a field of boxes.
    let (setback_height, setback_scale) = if height > 26.0 && rng.next_f32() < 0.75 {
        (
            height * SETBACK_RATIO * rng.range_f32(0.7, 1.0),
            rng.range_f32(0.45, 0.78),
        )
    } else {
        (0.0, 0.0)
    };
    let floor_height = rng.range_f32(2.9, 3.6);
    let floors = ((height / floor_height).floor() as i32).clamp(1, u8::MAX as i32) as u8;
    let height = apply_downtown(params, height, downtown, rng);
    let landmark = height > params.height_max + params.landmark_extra * 0.35;
    let size = plot.size();
    let variant = (rng.next_f32() * 8.0) as u8;
    Building {
        id: 0, // stamped by the caller's collection index
        block,
        footprint: plot,
        height,
        setback_height,
        setback_scale,
        style,
        roof: pick_roof(height, rng),
        floor_height,
        floors,
        windows_x: window_columns(size.x),
        windows_z: window_columns(size.y),
        shopfront: style.wants_shopfront(floor_count(height, params)),
        variant,
        seed: rng.next_u64(),
        landmark,
        accent: Building::NEON_PALETTE
            [(rng.next_u64() % Building::NEON_PALETTE.len() as u64) as usize],
    }
}

/// Storeys implied by a height, clamped to what a facade can show.
fn floor_count(height: f32, _params: &CityParams) -> u8 {
    ((height / 3.2).floor() as i32).clamp(1, u8::MAX as i32) as u8
}
