//! Flat-shading colours of the city (no textures anywhere: every surface is a constant).
//!
//! Values are linear-ish `0..=1` RGB triples fed straight into the vertex colour slot.

/// Asphalt carriageway.
pub const ASPHALT: [f32; 3] = [0.17, 0.18, 0.21];
/// Pedestrian paving.
pub const SIDEWALK: [f32; 3] = [0.34, 0.35, 0.38];
pub const PARK: [f32; 3] = [0.14, 0.36, 0.17];
pub const PLAZA: [f32; 3] = [0.42, 0.40, 0.38];
pub const LOT: [f32; 3] = [0.24, 0.27, 0.27];
pub const ROOF: [f32; 3] = [0.36, 0.37, 0.41];
pub const TRUNK: [f32; 3] = [0.27, 0.19, 0.12];
pub const LEAF: [f32; 3] = [0.16, 0.42, 0.19];
pub const METAL: [f32; 3] = [0.44, 0.46, 0.50];
pub const LAMP_ON: [f32; 3] = [1.00, 0.85, 0.52];
pub const MONUMENT: [f32; 3] = [0.60, 0.58, 0.66];
pub const CONCRETE: [f32; 3] = [0.40, 0.41, 0.45];
/// Road markings (worn white and centre yellow).
pub const PAINT_WHITE: [f32; 3] = [0.82, 0.83, 0.84];
pub const PAINT_YELLOW: [f32; 3] = [0.76, 0.64, 0.26];
/// Park / lot furniture.
pub const BENCH_WOOD: [f32; 3] = [0.44, 0.30, 0.18];
pub const BIN_GREEN: [f32; 3] = [0.20, 0.34, 0.24];
pub const HYDRANT: [f32; 3] = [0.68, 0.20, 0.18];
pub const PARKING_STRIPE: [f32; 3] = [0.70, 0.71, 0.72];

/// Facade palettes indexed by `Building::variant` (six painted plasters / bricks).
pub const FACADES: [[f32; 3]; 6] = [
    [0.56, 0.45, 0.37],
    [0.63, 0.60, 0.55],
    [0.42, 0.52, 0.62],
    [0.68, 0.52, 0.44],
    [0.47, 0.49, 0.56],
    [0.58, 0.58, 0.54],
];

/// Facade colour derived from a building's procedural variant (landmarks read a touch
/// brighter so a skyline gets a hero tower).
pub fn facade_color(building: &city_layout::Building) -> [f32; 3] {
    let c = FACADES[(building.variant as usize) % FACADES.len()];
    if building.landmark {
        [c[0] * 1.05, c[1] * 1.02, c[2] * 1.1]
    } else {
        c
    }
}

/// Shirt colours, indexed by `Ped::variant`.
pub const SHIRTS: [[f32; 3]; 8] = [
    [0.72, 0.28, 0.30],
    [0.30, 0.48, 0.72],
    [0.82, 0.70, 0.30],
    [0.30, 0.58, 0.62],
    [0.58, 0.56, 0.32],
    [0.34, 0.34, 0.62],
    [0.72, 0.42, 0.62],
    [0.46, 0.48, 0.52],
];
/// Trousers are darker than the shirt so the figure reads as two parts.
pub const TROUSERS: [f32; 3] = [0.20, 0.23, 0.30];
/// Head / skin tone base.
pub const SKIN: [f32; 3] = [0.78, 0.62, 0.48];
/// Hair cap.
pub const HAIR: [f32; 3] = [0.16, 0.14, 0.16];
/// Car paint, indexed by `Car::variant`.
pub const PAINT: [[f32; 3]; 6] = [
    [0.70, 0.22, 0.24],
    [0.24, 0.36, 0.60],
    [0.80, 0.72, 0.34],
    [0.72, 0.70, 0.66],
    [0.24, 0.48, 0.40],
    [0.30, 0.30, 0.34],
];
/// Taxi yellow.
pub const TAXI: [f32; 3] = [0.86, 0.66, 0.14];
/// Cab glass.
pub const GLASS: [f32; 3] = [0.16, 0.24, 0.34];
/// Tail lamps.
pub const TAIL: [f32; 3] = [0.90, 0.18, 0.14];
/// Head lamps.
pub const LAMP: [f32; 3] = [1.00, 0.94, 0.74];
/// Tyres / underbody.
pub const RUBBER: [f32; 3] = [0.10, 0.11, 0.16];

/// Scale a colour by a deterministic per-agent factor (variant variation).
#[inline]
pub fn tint(c: [f32; 3], variant: u8) -> [f32; 3] {
    let k = 0.88 + 0.04 * ((variant as usize) % 4) as f32;
    [c[0] * k, c[1] * k, c[2] * k]
}

/// Blend two colours (`t = 0` ⇒ `a`).
#[inline]
pub fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = city_math::saturate(t);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Scale a colour down.
#[inline]
pub fn shade(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}
