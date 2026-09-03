//! Buildings: massing, facade style and roof furniture.
//!
//! A [`Building`] is a box plus the metadata the renderer needs to shade its
//! facade procedurally (window grid, banding, shopfront, neon accents). Placement
//! lives in [`crate::generate`].

use city_math::{Aabb2, Vec2};

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
            (base + 0.35).min(0.95)
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
