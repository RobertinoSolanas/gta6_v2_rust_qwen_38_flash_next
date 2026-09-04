//! Base colours of the generated materials.
//!
//! These mirror (and intentionally replace) the flat palette that `city-app` used
//! before textures existed — the painters multiply and blend them with noise, so the
//! overall palette of the city stays the same, only its surface detail is new.

/// Base asphalt (carriageway bitumen).
pub const ASPHALT: [u8; 3] = [46, 47, 54];
/// Dark void between aggregate grains.
pub const ASPHALT_DARK: [u8; 3] = [24, 25, 30];
/// Bright aggregate grains in asphalt.
pub const ASPHALT_GRAIN: [u8; 3] = [92, 92, 99];
/// Oil stain on the carriageway.
pub const OIL_STAIN: [u8; 3] = [16, 16, 20];

/// Concrete (kerbs, block caps).
pub const CONCRETE: [u8; 3] = [104, 106, 115];
/// Darker concrete speck/stain.
pub const CONCRETE_DARK: [u8; 3] = [78, 80, 88];
/// Sidewalk slab concrete.
pub const SIDEWALK: [u8; 3] = [134, 136, 140];
/// Sidewalk slab joint / grout.
pub const SIDEWALK_JOINT: [u8; 3] = [56, 58, 64];
/// Sidewalk crack colour.
pub const SIDEWALK_CRACK: [u8; 3] = [40, 41, 46];
/// Chewing-gum / drip stain on the sidewalk.
pub const SIDEWALK_STAIN: [u8; 3] = [86, 84, 72];

/// Grass shadow tone.
pub const GRASS: [u8; 3] = [40, 96, 46];
/// Grass highlight.
pub const GRASS_LIGHT: [u8; 3] = [76, 138, 66];
/// Dry / straw patch.
pub const GRASS_DRY: [u8; 3] = [122, 116, 52];
/// Deepest grass shadow.
pub const GRASS_DARK: [u8; 3] = [26, 62, 34];

/// Brick body mid tone.
pub const BRICK: [u8; 3] = [142, 78, 58];
/// Brick shadow tone (mixed per-brick).
pub const BRICK_DARK: [u8; 3] = [96, 50, 42];
/// Brick light tone.
pub const BRICK_LIGHT: [u8; 3] = [176, 106, 78];
/// Overfired "burnt" brick.
pub const BRICK_BURNT: [u8; 3] = [70, 44, 40];
/// Brick mortar joint.
pub const MORTAR: [u8; 3] = [168, 158, 146];

/// Plaster / rendered facade base.
pub const PLASTER: [u8; 3] = [168, 160, 148];
/// Plaster stain (soot, rain streaks).
pub const PLASTER_DARK: [u8; 3] = [120, 114, 106];

/// Roof gravel dark bitumen binding.
pub const GRAVEL_DARK: [u8; 3] = [66, 66, 74];
/// Roof gravel chippings.
pub const GRAVEL_LIGHT: [u8; 3] = [132, 132, 140];
/// Bright chipping flecks on the roof.
pub const GRAVEL_BRIGHT: [u8; 3] = [172, 172, 140];
/// Wet / puddled gravel sheen.
pub const GRAVEL_WET: [u8; 3] = [44, 46, 54];

/// Metal (poles, lamps, bus shelters).
pub const METAL: [u8; 3] = [112, 117, 128];
/// Rust freckles on metal.
pub const RUST: [u8; 3] = [122, 74, 40];

/// Road marking paint (white).
pub const PAINT_WHITE: [u8; 3] = [222, 224, 226];
/// Road marking paint (centre yellow).
pub const PAINT_YELLOW: [u8; 3] = [212, 176, 62];
