//! The pedestrian network.
//!
//! Every block owns one [`SidewalkLoop`]: a closed polyline running along the middle
//! of its sidewalk band. Loops of neighbouring blocks are joined by
//! [`CrossingLink`]s placed at the zebra crossings, so a pedestrian can wander the
//! whole city without cutting through a building or jaywalking.

use city_math::{Aabb2, Seg2, Vec2};

use crate::params::CityParams;
use crate::{Crossing, Road};

/// One closed sidewalk loop around a block.
#[derive(Clone, Debug)]
pub struct SidewalkLoop {
    /// Id inside [`crate::City::loops`].
    pub id: usize,
    /// Block this sidewalk belongs to (`usize::MAX` for the outer ring walk).
    pub block: usize,
    /// Waypoints in clockwise order.
    pub points: Vec<Vec2>,
    /// Cumulative arc length at each waypoint (`cum[0] == 0.0`).
    pub cum: Vec<f32>,
    /// Total perimeter in metres.
    pub length: f32,
}

impl SidewalkLoop {
    /// Perimeter of the loop.
    #[inline]
    pub fn perimeter(&self) -> f32 {
        self.length
    }

    /// Number of waypoints.
    #[inline]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// `true` when the loop has no waypoints.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Waypoints (read-only).
    #[inline]
    pub fn points(&self) -> &[Vec2] {
        &self.points
    }

    /// Wrap an arc length into `[0, length)`.
    #[inline]
    pub fn wrap(&self, s: f32) -> f32 {
        city_math::wrap_period(s, self.length)
    }

    /// Index of the segment starting before arc length `s`.
    pub fn segment_at(&self, s: f32) -> usize {
        let s = self.wrap(s);
        if self.cum.len() < 2 {
            return 0;
        }
        let mut idx = 0usize;
        for (i, &c) in self.cum.iter().enumerate() {
            if c <= s {
                idx = i;
            } else {
                break;
            }
        }
        idx
    }

    /// Position at arc length `s` (wrapped).
    pub fn point_at(&self, s: f32) -> Vec2 {
        let n = self.points.len();
        if n < 2 {
            return self.points.first().copied().unwrap_or(Vec2::ZERO);
        }
        let s = self.wrap(s);
        let i = self.segment_at(s);
        let j = (i + 1) % n;
        let a = self.points[i];
        let b = self.points[j];
        let seg = (b - a).len();
        if seg < 1e-6 {
            return a;
        }
        let t = city_math::clamp((s - self.cum[i]) / seg, 0.0, 1.0);
        a.lerp(b, t)
    }

    /// Walking direction at arc length `s`.
    pub fn dir_at(&self, s: f32) -> Vec2 {
        let d = self.point_at(s + 0.4) - self.point_at(s - 0.4);
        if d.len_sq() < 1e-8 {
            Vec2::X
        } else {
            d.norm()
        }
    }

    /// Closest point on the loop: `(position, arc length, distance)`.
    pub fn project(&self, p: Vec2) -> (Vec2, f32, f32) {
        let n = self.points.len();
        if n < 2 {
            let only = self.points.first().copied().unwrap_or(Vec2::ZERO);
            return (only, 0.0, p.dist(only));
        }
        let mut best: Option<(f32, Vec2, f32)> = None;
        for i in 0..n {
            let a = self.points[i];
            let b = self.points[(i + 1) % n];
            let (q, _t) = Seg2::new(a, b).closest(p);
            let d = p.dist(q);
            if best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
                best = Some((d, q, self.cum[i] + a.dist(q)));
            }
        }
        match best {
            Some((d, q, s)) => (q, self.wrap(s), d),
            None => (p, 0.0, 0.0),
        }
    }
}

/// A link that lets pedestrians cross one carriageway between two loops.
#[derive(Clone, Debug)]
pub struct CrossingLink {
    /// Id inside [`crate::City::links`].
    pub id: usize,
    /// Loop on the near side.
    pub from_loop: usize,
    /// Loop on the far side.
    pub to_loop: usize,
    /// Arc length on `from_loop` where the crossing starts.
    pub from_s: f32,
    /// Arc length on `to_loop` where the crossing lands.
    pub to_s: f32,
    /// World mid point of the crossing.
    pub mid: Vec2,
}

/// Build one [`SidewalkLoop`] per block plus the outer ring walk.
pub fn build_loops(params: &CityParams, loops: &mut Vec<SidewalkLoop>) {
    let inset = params.sidewalk_width * 0.5;
    let spacing = 4.0;

    for iz in 0..params.blocks_z {
        for ix in 0..params.blocks_x {
            let bounds = Aabb2::from_min_size(
                Vec2::new(params.block_min(ix), params.block_min(iz)),
                Vec2::new(params.block_size, params.block_size),
            );
            let (points, length) = rect_polyline(bounds, inset, spacing);
            let id = loops.len();
            loops.push(SidewalkLoop {
                id,
                block: id,
                cum: cumulative(&points),
                points,
                length,
            });
        }
    }

    // Outer ring walk around the whole city.
    let (points, length) = rect_polyline(params.city_bounds(), 0.7, spacing);
    let id = loops.len();
    loops.push(SidewalkLoop {
        id,
        block: usize::MAX,
        cum: cumulative(&points),
        points,
        length,
    });
}

/// Rectangle polyline inset by `inset`, sampled every `spacing` metres.
///
/// Returns the clockwise waypoints (starting near the lower-left corner) and the
/// total perimeter.
pub fn rect_polyline(bounds: Aabb2, inset: f32, spacing: f32) -> (Vec<Vec2>, f32) {
    let inner = bounds.grown(-inset.max(0.0));
    let (min, max) = (inner.min, inner.max);
    let corners = [
        min,
        Vec2::new(max.x, min.y),
        Vec2::new(max.x, max.y),
        Vec2::new(min.x, max.y),
    ];
    let mut points = Vec::new();
    let mut total = 0.0f32;
    for i in 0..4 {
        let a = corners[i];
        let b = corners[(i + 1) % 4];
        let seg = a.dist(b);
        if seg <= 0.01 {
            continue;
        }
        let steps = ((seg / spacing.max(0.5)).ceil() as usize).max(1);
        for s in 0..steps {
            points.push(a.lerp(b, s as f32 / steps as f32));
        }
        total += seg;
    }
    if points.is_empty() {
        points.push(inner.center());
    }
    (points, total)
}

/// Cumulative arc lengths of a polyline (first entry is `0.0`).
pub fn cumulative(points: &[Vec2]) -> Vec<f32> {
    let mut cum = Vec::with_capacity(points.len());
    let mut acc = 0.0f32;
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            acc += points[i - 1].dist(*p);
        }
        cum.push(acc);
    }
    cum
}

/// Loop id of block `(ix, iz)`, or the outer ring id when outside the grid.
#[inline]
pub fn loop_id(params: &CityParams, ix: i32, iz: i32) -> usize {
    let ring = params.blocks_x * params.blocks_z;
    if ix < 0 || iz < 0 || ix as usize >= params.blocks_x || iz as usize >= params.blocks_z {
        ring
    } else {
        ix as usize * params.blocks_z + iz as usize
    }
}

/// Links between the loops on either side of every crossing.
pub fn build_links(
    params: &CityParams,
    loops: &[SidewalkLoop],
    roads: &[Road],
    crossings: &[Crossing],
    out: &mut Vec<CrossingLink>,
) {
    for c in crossings {
        let Some(road) = roads.get(c.road) else {
            continue;
        };
        // The face's grid line and its position along the line.
        let line = road.line as i32;
        let face = face_of(params, road);
        let (a, b) = match road.axis {
            crate::roads::Axis::NorthSouth => {
                (loop_id(params, line - 1, face), loop_id(params, line, face))
            }
            crate::roads::Axis::EastWest => {
                (loop_id(params, face, line - 1), loop_id(params, face, line))
            }
        };
        if a == b || a >= loops.len() || b >= loops.len() {
            continue;
        }
        let near = c.center - c.dir * (c.length * 0.5 + 0.5);
        let far = c.center + c.dir * (c.length * 0.5 + 0.5);
        let from_s = loops[a].project(near).1;
        let to_s = loops[b].project(far).1;
        out.push(CrossingLink {
            id: out.len(),
            from_loop: a,
            to_loop: b,
            from_s,
            to_s,
            mid: c.center,
        });
    }
}

/// Index of the block face a road segment sits next to (along its own axis).
fn face_of(params: &CityParams, road: &Road) -> i32 {
    let from = crate::roads::node_cell(params, road.from_node);
    let to = crate::roads::node_cell(params, road.to_node);
    if road.axis.is_x() {
        from[0].min(to[0]) as i32
    } else {
        from[1].min(to[1]) as i32
    }
}
