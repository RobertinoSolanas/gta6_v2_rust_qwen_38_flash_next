//! # city-mesh
//!
//! Geometry builders: everything the renderer draws, built on the CPU from the data the
//! other contexts produce — the static city ([`city_layout::City`]), the crowd
//! ([`city_sim::Ped`] / [`city_sim::Car`]) and the player's body
//! ([`city_avatar::AvatarPose`]).
//!
//! Layout:
//! * [`builder`] — the append-only [`MeshBuilder`] and its vertex
//!   format (position + normal + colour, the format the live GL path uploads verbatim).
//! * [`palette`] — flat-shading colours shared by every builder.
//! * [`city`] — ground, kerbs, buildings, street furniture, road markings.
//! * [`humanoid`] — the humanoid **part palette**: the part list and part-local bind
//!   poses that make an animated figure out of boxes.
//! * [`agents`] — pedestrians and cars, with the walk cycle driven by the agent's own
//!   stride phase.
//!
//! Design rules:
//! * Pure data in, `Vec<f32>` out. No DOM, no GL, natively testable.
//! * The humanoid is *rigged*, not modelled: 11 boxes with named local transforms,
//!   posed by a [`PartPose`] — one palette, cheap to instance.

#![forbid(unsafe_code)]

pub mod agents;
pub mod builder;
pub mod city;
pub mod humanoid;
pub mod palette;

pub use agents::{build_agents, car, ped};
pub use builder::{vertex_count, MeshBuilder, FLOATS_PER_VERTEX};
pub use city::{
    block_surface_color, build_city, build_parking_stripes, build_road_markings, building_mesh,
    prop, zebra_bars, zebra_crossing,
};
pub use humanoid::{
    figure_colors, part_bind_frames, part_frames, part_local, Bone, PartPose, FIGURE_HEIGHT,
    HIP_HEIGHT, PART_COUNT, PART_GEOM, PART_ORDER,
};
pub use palette::facade_color;

/// Material id attached to a piece of geometry (the renderer's texture-slot hint, the
/// same order as `city_tex::ALL_MATERIALS`). Kept as a plain `u8` here so this crate
/// stays free of the texture crate: the two orders are pinned equal by a test.
pub mod slot {
    /// Sampler slot order of `city_tex::ALL_MATERIALS`.
    pub const ASPHALT: u8 = 0;
    pub const CONCRETE: u8 = 1;
    pub const SIDEWALK: u8 = 2;
    pub const GRASS: u8 = 3;
    pub const BRICK: u8 = 4;
    pub const PLASTER: u8 = 5;
    pub const ROOF_GRAVEL: u8 = 6;
    pub const METAL: u8 = 7;
    pub const PAINT_WHITE: u8 = 8;
    pub const ROAD_LINE_YELLOW: u8 = 9;
}
