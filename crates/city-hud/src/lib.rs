//! # city-hud
//!
//! Bounded context for *what the player is told*: the minimap's rotated primitives, the
//! clock / compass / speed strings and the context tip line. Pure data — `city-app` draws
//! the result with Canvas2D; nothing in here touches the DOM.
//!
//! Design notes:
//! * The radar either keeps north up or rotates so the view direction points to the top
//!   of the dial. Either way every primitive ends up in *radar space* `-1..=1`
//!   (`+y` = up on screen), so the back end only multiplies by the radar radius.
//! * Geometry is distance-filtered **before** projection, which keeps a frame tiny
//!   (a dozen lines and a handful of dots on the default city).
//! * Text is assembled here, so the drawing back end never formats a string.

#![forbid(unsafe_code)]

use city_layout::{Block, BlockKind, City, PropKind};
use city_math::{saturate, wrap_period, Vec2};
use city_sky::SkySample;

/// World metres from the radar centre to its rim.
pub const RADAR_RANGE: f32 = 110.0;

/// A line in radar space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudLine {
    pub a: [f32; 2],
    pub b: [f32; 2],
    /// Width in pixels.
    pub width: f32,
    /// `true` for the wider arterials.
    pub avenue: bool,
}

/// A point marker in radar space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudDot {
    pub p: [f32; 2],
    /// Radius in pixels.
    pub size: f32,
    pub kind: HudDotKind,
}

/// Marker categories (each has its own colour in the back end).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudDotKind {
    Player,
    Green,
    Landmark,
    Lamp,
}

/// Everything the overlay needs for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct HudFrame {
    pub lines: Vec<HudLine>,
    pub dots: Vec<HudDot>,
    /// `HH:MM`.
    pub clock: String,
    /// `day` / `dusk` / `night` / …
    pub phase: String,
    /// Compass letter of the view direction.
    pub compass: String,
    /// Speed in km/h, rounded to 0.1.
    pub speed_kmh: f32,
    pub sprinting: bool,
    /// Distance walked since spawn (m).
    pub walked: f32,
    /// Active camera preset, 1-based for display.
    pub cam_index: usize,
    pub cam_count: usize,
    /// Pointer captured?
    pub locked: bool,
    /// Day-phase skip running?
    pub skipping: bool,
    /// Context line (empty = draw nothing).
    pub tip: String,
    /// Seconds since boot.
    pub uptime: f32,
}

impl Default for HudFrame {
    fn default() -> Self {
        HudFrame {
            lines: Vec::new(),
            dots: Vec::new(),
            clock: String::new(),
            phase: String::new(),
            compass: String::from("E"),
            speed_kmh: 0.0,
            sprinting: false,
            walked: 0.0,
            cam_index: 1,
            cam_count: 4,
            locked: false,
            skipping: false,
            tip: String::new(),
            uptime: 0.0,
        }
    }
}

/// World → radar transform.
#[derive(Clone, Copy, Debug)]
pub struct Radar {
    cx: f32,
    cz: f32,
    rot: f32,
    scale: f32,
    north_up: bool,
}

impl Radar {
    /// Radar centred on `center`. `north_up == false` rotates the world so the view
    /// direction (`yaw`) points to the top of the dial.
    pub fn new(center: Vec2, yaw: f32, range: f32, north_up: bool) -> Radar {
        let angle = if north_up { 0.0 } else { -yaw };
        Radar {
            cx: center.x,
            cz: center.y,
            rot: angle,
            scale: 1.0 / safe_range(range),
            north_up,
        }
    }

    /// Project a world point into radar space.
    #[inline]
    pub fn project(&self, p: Vec2) -> [f32; 2] {
        let d = (p - Vec2::new(self.cx, self.cz)) * self.scale;
        let (s, c) = self.rot.sin_cos();
        let x = d.x * c - d.y * s;
        let y = d.x * s + d.y * c;
        [x, -y] // radar space: +y is up, world +Z points down the dial
    }

    /// `true` when `p` is off the dial.
    #[inline]
    pub fn outside(&self, p: Vec2) -> bool {
        let q = self.project(p);
        q[0] * q[0] + q[1] * q[1] > 1.0
    }

    /// `true` when a whole segment misses the dial (cheap conservative cull).
    pub fn segment_outside(&self, a: Vec2, b: Vec2) -> bool {
        let pa = self.project(a);
        let pb = self.project(b);
        if pa[0] * pa[0] + pa[1] * pa[1] <= 1.0 || pb[0] * pb[0] + pb[1] * pb[1] <= 1.0 {
            return false;
        }
        segment_misses_unit_disc(pa, pb)
    }

    /// Radar radius in world metres.
    #[inline]
    pub fn range(&self) -> f32 {
        1.0 / self.scale
    }

    #[inline]
    pub fn is_north_up(&self) -> bool {
        self.north_up
    }
}

/// Inputs of one HUD frame (assembled by `city-app`).
pub struct HudInput<'a> {
    pub city: &'a City,
    pub pos: Vec2,
    pub yaw: f32,
    pub speed: f32,
    pub sprinting: bool,
    pub walked: f32,
    pub cam_index: usize,
    pub cam_count: usize,
    pub locked: bool,
    pub skipping: bool,
    pub clock: String,
    pub phase: String,
    pub range: f32,
    pub north_up: bool,
    pub uptime: f32,
    /// Context tip (see [`context_tip`]).
    pub tip: String,
}

/// Build one frame of HUD data.
pub fn build(input: &HudInput) -> HudFrame {
    let city = input.city;
    let radar = Radar::new(input.pos, input.yaw, input.range, input.north_up);
    let mut f = HudFrame {
        clock: input.clock.clone(),
        phase: input.phase.clone(),
        compass: compass(input.yaw),
        speed_kmh: round1(speed_kmh(input.speed)),
        sprinting: input.sprinting,
        walked: round1(input.walked),
        cam_index: input.cam_index + 1,
        cam_count: input.cam_count,
        locked: input.locked,
        skipping: input.skipping,
        tip: input.tip.clone(),
        uptime: input.uptime,
        lines: Vec::new(),
        dots: Vec::new(),
    };

    // carriageways
    for road in city.roads() {
        let line = road.center_line(city.params());
        if radar.segment_outside(line.a, line.b) {
            continue;
        }
        let avenue = matches!(road.kind, city_layout::RoadKind::Avenue);
        f.lines.push(HudLine {
            a: radar.project(line.a),
            b: radar.project(line.b),
            width: if avenue { 2.6 } else { 1.4 },
            avenue,
        });
    }

    // painted crossings: short ticks across the carriageway
    for crossing in city.crossings() {
        if radar.outside(crossing.center) {
            continue;
        }
        let half = crossing_half_width(crossing);
        f.lines.push(HudLine {
            a: radar.project(crossing_end(crossing, -half)),
            b: radar.project(crossing_end(crossing, half)),
            width: 1.0,
            avenue: false,
        });
    }

    // green / plaza blocks
    for block in city.blocks() {
        if !is_green(block.kind) {
            continue;
        }
        let c = block_center(block);
        if radar.outside(c) {
            continue;
        }
        f.dots.push(HudDot {
            p: radar.project(c),
            size: dot_size(HudDotKind::Green),
            kind: HudDotKind::Green,
        });
    }

    // props worth a marker
    for prop in city.props() {
        let kind = match prop.kind {
            PropKind::Monument => HudDotKind::Landmark,
            PropKind::Lamp if prop.glow > 0.1 => HudDotKind::Lamp,
            _ => continue,
        };
        if radar.outside(prop.pos) {
            continue;
        }
        f.dots.push(HudDot {
            p: radar.project(prop.pos),
            size: dot_size(kind),
            kind,
        });
    }

    // the player is always dead centre
    f.dots.push(HudDot {
        p: [0.0, 0.0],
        size: dot_size(HudDotKind::Player),
        kind: HudDotKind::Player,
    });

    f
}

/// 8-way compass letter. World convention: `+X` = east, `-Z` = north.
pub fn compass(yaw: f32) -> String {
    const NAMES: [&str; 8] = ["E", "SE", "S", "SW", "W", "NW", "N", "NE"];
    let a = wrap_period(yaw, core::f32::consts::TAU);
    let idx = ((a / (core::f32::consts::TAU / 8.0) + 0.5).floor() as i32).rem_euclid(8) as usize;
    NAMES[idx].to_string()
}

/// m/s → km/h.
#[inline]
pub fn speed_kmh(speed: f32) -> f32 {
    speed.max(0.0) * 3.6
}

/// Round to one decimal so the read-out does not flicker.
#[inline]
pub fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

/// Where am I? — one human readable line.
pub fn context_tip(city: &City, pos: Vec2, sky: &SkySample) -> String {
    let kind = city.block_at(pos).map(|b| b.kind);
    let mut tip = match kind {
        Some(BlockKind::Park) => String::from("Standing in the park"),
        Some(BlockKind::Plaza) => String::from("On the plaza"),
        Some(BlockKind::Lot) => String::from("In the surface car park"),
        Some(BlockKind::Urban) => {
            if city.distance_to_road(pos) < 1.2 {
                String::from("On the carriageway")
            } else {
                String::from("On the sidewalk")
            }
        }
        None => String::from("At the city edge"),
    };
    if sky.lamp_light > 0.85 {
        tip_push_lamps(&mut tip, sky);
    }
    tip
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

#[inline]
fn crossing_end(c: &city_layout::Crossing, k: f32) -> Vec2 {
    c.center + c.dir * k
}

#[inline]
fn crossing_half_width(c: &city_layout::Crossing) -> f32 {
    c.width * 0.5
}

#[inline]
fn block_center(block: &Block) -> Vec2 {
    block.bounds.center()
}

#[inline]
fn is_green(kind: BlockKind) -> bool {
    matches!(kind, BlockKind::Park | BlockKind::Plaza)
}

/// Pixel size of each marker kind.
pub fn dot_size(kind: HudDotKind) -> f32 {
    match kind {
        HudDotKind::Player => 4.5,
        HudDotKind::Green => 5.5,
        HudDotKind::Landmark => 4.0,
        HudDotKind::Lamp => 1.6,
    }
}

#[inline]
fn safe_range(range: f32) -> f32 {
    if range.is_finite() && range > 1.0 {
        range
    } else {
        RADAR_RANGE
    }
}

/// `true` when the segment between two *outside* points misses the unit disc.
fn segment_misses_unit_disc(a: [f32; 2], b: [f32; 2]) -> bool {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len_sq = d[0] * d[0] + d[1] * d[1];
    if len_sq < 1e-9 {
        return true;
    }
    let t = (-(a[0] * d[0] + a[1] * d[1]) / len_sq).clamp(0.0, 1.0);
    let cx = a[0] + d[0] * t;
    let cy = a[1] + d[1] * t;
    cx * cx + cy * cy > 1.0
}

/// Append the night hint when the street lights are on.
#[inline]
fn tip_push_lamps(tip: &mut String, sky: &SkySample) {
    if sky.headlight > 0.6 {
        tip.push_str(" · street lights on");
    }
}

/// Radar fill alpha: brighter near the centre, faded at the rim.
#[inline]
pub fn rim_fade(v: [f32; 2]) -> f32 {
    let d = (v[0] * v[0] + v[1] * v[1]).sqrt();
    saturate(1.0 - d)
}
