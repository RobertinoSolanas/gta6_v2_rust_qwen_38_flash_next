//! Dynamic geometry: the pedestrians and cars of [`city_sim`], rebuilt every frame.
//!
//! The static city is a static VBO; the crowd is not, so it goes into its own dynamic
//! buffer. Until `city-mesh` ships the humanoid rig (increment I7) a walker is drawn as
//! a three-part block figure — legs, body, head — and a car as a body, a glasshouse and
//! two light bars. Everything is axis-aligned boxes in the existing vertex format, so
//! the crowd costs a few hundred triangles per frame and needs no new shader.

use city_sim::{Car, CarKind, Ped};

use crate::mesh::{MeshBuilder, palette};

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
/// Head colour.
pub const SKIN: [f32; 3] = [0.78, 0.62, 0.48];
/// Car paint, indexed by `Car::variant`.
pub const PAINT: [[f32; 3]; 6] = [
    [0.70, 0.22, 0.24],
    [0.24, 0.36, 0.60],
    [0.80, 0.72, 0.34],
    [0.72, 0.70, 0.66],
    [0.24, 0.48, 0.40],
    [0.30, 0.30, 0.34],
];
/// Cab glass.
pub const GLASS: [f32; 3] = [0.16, 0.24, 0.34];
/// Tail lamps (emissive at night through the lamp curve of `city-sky`).
pub const TAIL: [f32; 3] = [0.90, 0.18, 0.14];
/// Head lamps.
pub const LAMP: [f32; 3] = [1.00, 0.94, 0.74];

/// Height of a pedestrian (m).
pub const PED_HEIGHT: f32 = 1.75;

/// Append one pedestrian: legs, body and head, painted from its variation tag.
pub fn ped(ped: &Ped, m: &mut MeshBuilder) {
    let shirt = tint(SHIRTS[(ped.variant as usize) % SHIRTS.len()], ped.variant);
    let skin = tint(SKIN, ped.variant);
    let (x, z) = (ped.x, ped.z);
    // legs (0.3 m), torso (up to 1.42 m), head on top
    m.box_shaded(
        [x - 0.14, 0.15, z - 0.14],
        [x + 0.14, 0.45, z + 0.14],
        TROUSERS,
        TROUSERS,
    );
    m.box_shaded(
        [x - 0.22, 0.45, z - 0.15],
        [x + 0.22, 1.42, z + 0.22],
        shirt,
        shirt,
    );
    m.box_shaded(
        [x - 0.11, 1.42, z - 0.11],
        [x + 0.11, PED_HEIGHT, z + 0.11],
        skin,
        skin,
    );
}

/// Append one car: body, cab and the two lamp bars (lamps bright at night).
pub fn car(car: &Car, headlight: f32, m: &mut MeshBuilder) {
    let (dx, dz) = (car.dir.x, car.dir.y);
    let half = car.kind.length() * 0.5;
    // Axis-aligned footprint: cars only drive along the two city axes.
    let ex = (half * dx.abs()).max(0.95);
    let ez = (half * dz.abs()).max(0.9);
    let paint = match car.kind {
        CarKind::Taxi => TAXI,
        _ => tint(PAINT[(car.variant as usize) % PAINT.len()], car.variant),
    };
    let body_h = if car.kind == CarKind::Van { 1.7 } else { 1.1 };
    m.box_shaded(
        [car.pos.x - ex, 0.25, car.pos.y - ez],
        [car.pos.x + ex, body_h, car.pos.y + ez],
        paint,
        paint,
    );
    // Cab: a shorter glass box towards the middle of the body.
    let (cx, cz) = (ex * 0.55, ez * 0.6);
    m.box_shaded(
        [car.pos.x - cx, body_h, car.pos.y - cz],
        [car.pos.x + cx, body_h + 0.5, car.pos.y + cz],
        GLASS,
        GLASS,
    );
    // Head lamps in front (they follow the headlight curve of `city-sky`), tail lamps
    // at the back of the body.
    let head = mix(TAIL, LAMP, headlight);
    let front = lamp_point(car, ex, ez, true);
    let back = lamp_point(car, ex, ez, false);
    m.box_shaded(
        [front.0 - 0.22, 0.4, front.1 - 0.4],
        [front.0 + 0.22, 0.8, front.1 + 0.3],
        head,
        head,
    );
    m.box_shaded(
        [back.0 - 0.22, 0.4, back.1 - 0.4],
        [back.0 + 0.22, 0.8, back.1 + 0.3],
        TAIL,
        TAIL,
    );
}

/// World point just in front of a car.
fn lamp_point(car: &Car, ex: f32, ez: f32, front: bool) -> (f32, f32) {
    let s = if front { 1.0 } else { -1.0 };
    (car.pos.x + car.dir.x * ex * s, car.pos.y + car.dir.y * ez * s)
}

/// Taxi yellow.
pub const TAXI: [f32; 3] = [0.86, 0.66, 0.14];

/// Scale a colour by a deterministic per-agent factor.
fn tint(c: [f32; 3], variant: u8) -> [f32; 3] {
    let k = 0.88 + 0.04 * ((variant as usize) % 4) as f32;
    [c[0] * k, c[1] * k, c[2] * k]
}

/// Blend two colours.
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = city_math::saturate(t);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// All agents of one frame.
pub fn build_agents(peds: &[city_sim::Ped], cars: &[Car], headlight: f32, m: &mut MeshBuilder) {
    for ped in peds {
        self::ped(ped, m);
    }
    for car in cars {
        self::car(car, headlight, m);
    }
    let _ = palette::ASPHALT;
}
