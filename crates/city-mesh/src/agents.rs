//! Dynamic geometry: the pedestrians and cars of `city-sim`, rebuilt every frame.
//!
//! The static city is a static VBO; the crowd is not, so it gets its own dynamic buffer.
//! A walker is the [`crate::humanoid`] part palette, animated by the agent's own stride
//! phase; a car is a body, a glasshouse and two lamp bars. Everything stays in the shared
//! vertex format, so the crowd costs a few hundred triangles per frame and needs no new
//! shader.

use city_sim::{Car, CarKind, Ped};

use crate::builder::MeshBuilder;
use crate::humanoid::{self, figure_colors, PartPose};
use crate::palette::{self, mix, tint};

/// Height of a pedestrian (m): the palette's standing height (1.84 m).
/// Height of a drawn pedestrian: sole to the top of the head box, which is where the
/// figure it draws actually ends (see [`humanoid::FIGURE_HEIGHT`]).
pub const PED_HEIGHT: f32 = humanoid::FIGURE_HEIGHT;

/// Uniform size multiplier of a crowd figure.
pub const PED_SCALE: f32 = 1.0;

/// Ground height of the block plateaus / kerbs (`city-layout` walks the pavements 15 cm
/// above the carriageway).
pub const KERB_GROUND: f32 = crate::city::KERB_HEIGHT;

/// Append one pedestrian, walk-cycled from its own stride phase.
///
/// `ground` is the height under the walker (block interiors sit 0.15 m up).
pub fn ped(m: &mut MeshBuilder, p: &Ped, ground: f32) {
    let moving = p.speed > 0.15;
    let amp = walk_amp(p.speed);
    let pose = if moving {
        PartPose::walk(p.phase / std::f32::consts::TAU, amp)
    } else {
        PartPose::default()
    };
    let shirt = palette::SHIRTS[(p.variant as usize) % palette::SHIRTS.len()];
    let colors = figure_colors(tint(shirt, p.variant), palette::TROUSERS, palette::SKIN);
    let frames =
        humanoid::figure_frames(p.pos().as_array(), p.dir.angle(), ground, PED_SCALE, &pose);
    humanoid::append_figure(m, &frames, &colors);
}

/// Walk amplitude for a walking speed: a shuffle at walking pace, saturating at sprint.
#[inline]
pub fn walk_amp(speed: f32) -> f32 {
    city_math::saturate(speed / SPRINT_SPEED)
}

/// Reference sprint speed used to scale the stride amplitude (m/s).
pub const SPRINT_SPEED: f32 = 6.0;

/// One box of the car emitter (6 faces).
pub const CAR_BOX_VERTS: usize = 36;
/// First vertex of the body box (the box order is underbody, body, cab, lamps).
pub const CAR_BODY_VERTS: usize = 36;
/// First vertex of the head-lamp bar.
pub const CAR_HEAD_LAMP_VERTS: usize = 3 * 36;
/// Vertices one car writes: underbody, body, cab and the two lamp bars.
pub const CAR_VERTS: usize = 5 * 36;

/// Append one car: underbody, body, cab and the two lamp bars.
///
/// `headlight` is the night curve from `city-sky` (`0` = off, `1` = full beam).
pub fn car(m: &mut MeshBuilder, c: &Car, headlight: f32) {
    let half = c.kind.length() * 0.5;
    // Cars only drive along the two city axes: an axis-aligned footprint is exact.
    let ex = (half * c.dir.x.abs()).max(0.95);
    let ez = (half * c.dir.y.abs()).max(0.9);
    let paint = match c.kind {
        CarKind::Taxi => palette::TAXI,
        _ => tint(
            palette::PAINT[(c.variant as usize) % palette::PAINT.len()],
            c.variant,
        ),
    };
    let body_h = match c.kind {
        CarKind::Van => 1.7,
        _ => 1.1,
    };
    let yaw = c.dir.angle();
    // underbody (dark, so the car does not float)
    m.box_shaded(
        [c.pos.x - ex, 0.15, c.pos.y - ez],
        [c.pos.x + ex, 0.32, c.pos.y + ez],
        palette::RUBBER,
        palette::RUBBER,
    );
    m.box_shaded(
        [c.pos.x - ex, 0.25, c.pos.y - ez],
        [c.pos.x + ex, body_h, c.pos.y + ez],
        paint,
        paint,
    );
    // cab: a shorter glass box towards the middle of the body
    let (cx, cz) = (ex * 0.55, ez * 0.6);
    m.box_shaded(
        [c.pos.x - cx, body_h, c.pos.y - cz],
        [c.pos.x + cx, body_h + 0.5, c.pos.y + cz],
        palette::GLASS,
        palette::GLASS,
    );
    // Head lamps in front, tail lamps at the back. During the day the front bar is a
    // grey glass block; the night curve turns it into a beam (`city_sky`'s headlight
    // curve), so a parked car at noon never glows.
    let head = mix(palette::GLASS, palette::LAMP, headlight);
    let front = lamp_point(c, ex, ez, true);
    let back = lamp_point(c, ex, ez, false);
    m.box_shaded(
        [front.0 - 0.22, 0.4, front.1 - 0.40],
        [front.0 + 0.22, 0.8, front.1 + 0.30],
        head,
        head,
    );
    m.box_shaded(
        [back.0 - 0.22, 0.4, back.1 - 0.40],
        [back.0 + 0.22, 0.8, back.1 + 0.30],
        palette::TAIL,
        palette::TAIL,
    );
    let _ = yaw;
}

/// World point just in front of (or behind) a car body.
fn lamp_point(car: &Car, ex: f32, ez: f32, front: bool) -> (f32, f32) {
    let s = if front { 1.0 } else { -1.0 };
    (
        car.pos.x + car.dir.x * ex * s,
        car.pos.y + car.dir.y * ez * s,
    )
}

/// Append every agent of one frame.
pub fn build_agents(peds: &[Ped], cars: &[Car], headlight: f32, ground: f32, m: &mut MeshBuilder) {
    for p in peds {
        self::ped(m, p, ground);
    }
    for c in cars {
        self::car(m, c, headlight);
    }
}
