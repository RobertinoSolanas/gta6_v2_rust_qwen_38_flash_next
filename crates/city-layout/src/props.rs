//! Props: the street furniture that makes a street feel inhabited.
//!
//! Props sit along the sidewalk centre line of every block, at spacings taken from
//! [`crate::CityParams`]. Only some of them are solid — a bench or a bin stops you
//! walking, a hydrant does not.

use city_math::{Aabb2, Vec2};

/// Kinds of street furniture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PropKind {
    /// Street tree (trunk + canopy).
    Tree = 0,
    /// Street lamp with an emissive head.
    Lamp = 1,
    /// Park bench.
    Bench = 2,
    /// Rubbish bin.
    Bin = 3,
    /// Fire hydrant.
    Hydrant = 4,
    /// Bus shelter (solid).
    BusStop = 5,
    /// Concrete barrier (solid).
    Barrier = 6,
    /// Planter box (solid).
    Planter = 7,
    /// Plaza pylon / monument (solid, glows at night).
    Monument = 8,
    /// Kerb bollard.
    Bollard = 9,
}

impl PropKind {
    /// Number of variants.
    pub const COUNT: usize = 10;

    /// All kinds, ordered by their discriminant.
    pub const ALL: [PropKind; 10] = [
        PropKind::Tree,
        PropKind::Lamp,
        PropKind::Bench,
        PropKind::Bin,
        PropKind::Hydrant,
        PropKind::BusStop,
        PropKind::Barrier,
        PropKind::Planter,
        PropKind::Monument,
        PropKind::Bollard,
    ];

    /// `true` when pedestrians must walk around it.
    #[inline]
    pub fn is_solid(self) -> bool {
        matches!(
            self,
            PropKind::BusStop
                | PropKind::Barrier
                | PropKind::Planter
                | PropKind::Monument
        )
    }

    /// Footprint size `(width, depth)` in metres, before `Prop::scale`.
    pub fn footprint_size(self) -> (f32, f32) {
        match self {
            PropKind::Tree => (0.8, 0.8),
            PropKind::Lamp => (0.5, 0.5),
            PropKind::Bench => (2.4, 0.9),
            PropKind::Bin => (0.85, 0.85),
            PropKind::Hydrant => (0.55, 0.55),
            PropKind::BusStop => (4.2, 1.7),
            PropKind::Barrier => (3.0, 0.9),
            PropKind::Planter => (1.6, 1.6),
            PropKind::Monument => (3.2, 3.2),
            PropKind::Bollard => (0.42, 0.42),
        }
    }

    /// Height in metres, before `Prop::scale`.
    pub fn height(self) -> f32 {
        match self {
            PropKind::Tree => 6.4,
            PropKind::Lamp => 7.2,
            PropKind::Bench => 0.95,
            PropKind::Bin => 1.2,
            PropKind::Hydrant => 0.85,
            PropKind::BusStop => 2.7,
            PropKind::Barrier => 0.9,
            PropKind::Planter => 0.95,
            PropKind::Monument => 11.0,
            PropKind::Bollard => 0.9,
        }
    }

    /// Human readable label (HUD tooltips, test messages).
    pub fn label(self) -> &'static str {
        match self {
            PropKind::Tree => "tree",
            PropKind::Lamp => "street lamp",
            PropKind::Bench => "bench",
            PropKind::Bin => "rubbish bin",
            PropKind::Hydrant => "fire hydrant",
            PropKind::BusStop => "bus stop",
            PropKind::Barrier => "barrier",
            PropKind::Planter => "planter",
            PropKind::Monument => "monument",
            PropKind::Bollard => "bollard",
        }
    }
}

/// A placed prop.
#[derive(Clone, Debug)]
pub struct Prop {
    /// Id inside [`crate::City::props`].
    pub id: usize,
    /// What it is.
    pub kind: PropKind,
    /// Base position on the ground plane.
    pub pos: Vec2,
    /// Facing angle in radians.
    pub yaw: f32,
    /// Per-instance size multiplier.
    pub scale: f32,
    /// Block the prop belongs to.
    pub block: usize,
    /// `true` when the prop casts a shadow.
    pub casts_shadow: bool,
    /// Night-time emissive strength (`0` = none).
    pub glow: f32,
}

impl Prop {
    /// World footprint centred on `pos`, rotated approximately (AABB approximation
    /// of the rotated box, which is what collision uses).
    pub fn world_footprint(&self) -> Aabb2 {
        let (w, d) = self.kind.footprint_size();
        let s = self.scale.max(0.1);
        let (sa, ca) = (self.yaw.sin().abs(), self.yaw.cos().abs());
        let hx = (w * sa + d * ca) * 0.5 * s;
        let hz = (w * ca + d * sa) * 0.5 * s;
        Aabb2::from_center_size(
            self.pos,
            Vec2::new(hx.max(0.15), hz.max(0.15)),
        )
    }

    /// Top of the prop in metres.
    #[inline]
    pub fn top(&self) -> f32 {
        self.kind.height() * self.scale
    }

    /// `true` when the player must walk around it.
    #[inline]
    pub fn blocks_walk(&self) -> bool {
        self.kind.is_solid()
    }
}
