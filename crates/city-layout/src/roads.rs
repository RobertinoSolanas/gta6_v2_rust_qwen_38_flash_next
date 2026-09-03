//! Streets: carriageways ([`Road`]), one [`Lane`] per direction per block face,
//! grid-node [`Intersection`]s and marked [`Crossing`]s.
//!
//! The network is built from *junction nodes*: node `(i, j)` sits at
//! `(road_center(i), road_center(j))`. Between two neighbouring nodes runs one
//! [`Road`] (a carriageway face) carrying two [`Lane`]s — one per direction, offset
//! to the right of travel (right-hand traffic). A lane spans exactly one block face,
//! so following the graph is simply `s → lane → lane.next`.

use city_math::{clamp, Seg2, Vec2};

use crate::params::CityParams;

/// Which way a road / lane runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Runs along +X.
    EastWest,
    /// Runs along +Z.
    NorthSouth,
}

impl Axis {
    /// The other axis.
    #[inline]
    pub const fn other(self) -> Axis {
        match self {
            Axis::EastWest => Axis::NorthSouth,
            Axis::NorthSouth => Axis::EastWest,
        }
    }

    /// Unit vector of the positive direction.
    #[inline]
    pub const fn positive(self) -> Vec2 {
        match self {
            Axis::EastWest => Vec2::X,
            Axis::NorthSouth => Vec2::Y,
        }
    }

    /// `true` when the axis varies along X.
    #[inline]
    pub const fn is_x(self) -> bool {
        matches!(self, Axis::EastWest)
    }

    /// Coordinate that varies along this axis.
    #[inline]
    pub fn along(self, p: Vec2) -> f32 {
        if self.is_x() {
            p.x
        } else {
            p.y
        }
    }

    /// Coordinate on the perpendicular axis.
    #[inline]
    pub fn across(self, p: Vec2) -> f32 {
        if self.is_x() {
            p.y
        } else {
            p.x
        }
    }

    /// Build a point from `along` / `across` coordinates.
    #[inline]
    pub fn point(self, along: f32, across: f32) -> Vec2 {
        if self.is_x() {
            Vec2::new(along, across)
        } else {
            Vec2::new(across, along)
        }
    }

    /// Traffic-light test for this axis at simulation time `time`.
    ///
    /// The first half of the cycle serves north-south traffic, the second half
    /// east-west traffic.
    #[inline]
    pub fn has_green(self, time: f32, phase: f32, cycle: f32, window: f32) -> bool {
        let half = cycle * 0.5;
        let t = city_math::wrap_period(time + phase, cycle);
        match self {
            Axis::NorthSouth => t < window,
            Axis::EastWest => t >= half && t < half + window,
        }
    }
}

/// Classification of a carriageway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadKind {
    /// Minor local street.
    Street,
    /// Major arterial (every `CityParams::major_period`-th line).
    Avenue,
}

/// A carriageway between two neighbouring junctions (one block face).
#[derive(Clone, Debug)]
pub struct Road {
    /// Id inside [`crate::City::roads`].
    pub id: usize,
    /// Orientation.
    pub axis: Axis,
    /// Grid line the road belongs to.
    pub line: usize,
    /// Centre line coordinate on the perpendicular axis.
    pub at: f32,
    /// Junction at the low end.
    pub from_node: usize,
    /// Junction at the far end.
    pub to_node: usize,
    /// Half of the carriageway width.
    pub half_width: f32,
    /// Street or avenue.
    pub kind: RoadKind,
    /// `[lane travelling +axis, lane travelling -axis]`.
    pub lanes: [usize; 2],
}

impl Road {
    /// Carriageway centre line (junction centre to junction centre).
    #[inline]
    pub fn center_line(&self, params: &CityParams) -> Seg2 {
        Seg2::new(
            self.axis.point(self.along_from(params), self.at),
            self.axis.point(self.along_to(params), self.at),
        )
    }

    /// `along` coordinate of the near junction.
    #[inline]
    pub fn along_from(&self, params: &CityParams) -> f32 {
        node_pos(params, self.from_node)
            .map(|p| self.axis.along(p))
            .unwrap_or(0.0)
    }

    /// `along` coordinate of the far junction.
    #[inline]
    pub fn along_to(&self, params: &CityParams) -> f32 {
        node_pos(params, self.to_node)
            .map(|p| self.axis.along(p))
            .unwrap_or(0.0)
    }

    /// `true` when a ground point is on the tarmac.
    #[inline]
    pub fn covers(&self, p: Vec2) -> bool {
        (self.axis.across(p) - self.at).abs() <= self.half_width
    }

    /// Speed limit in m/s.
    #[inline]
    pub fn speed_limit(&self) -> f32 {
        match self.kind {
            RoadKind::Street => 8.0,
            RoadKind::Avenue => 11.5,
        }
    }
}

/// One direction of a carriageway, spanning exactly one block face.
#[derive(Clone, Debug)]
pub struct Lane {
    /// Id inside [`crate::City::lanes`].
    pub id: usize,
    /// Owning road.
    pub road: usize,
    /// Direction of travel (unit).
    pub dir: Vec2,
    /// Exit of the upstream junction.
    pub start: Vec2,
    /// Entry of the downstream junction.
    pub end: Vec2,
    /// Lane length.
    pub length: f32,
    /// Speed limit in m/s.
    pub speed_limit: f32,
    /// `[upstream node, downstream node]`.
    pub nodes: [usize; 2],
    /// Lanes reachable at `nodes[1]` (straight on or turning).
    pub next: Vec<LaneTarget>,
}

impl Lane {
    /// Point at arc length `s` (clamped to the lane).
    #[inline]
    pub fn point_at(&self, s: f32) -> Vec2 {
        self.start + self.dir * clamp(s, 0.0, self.length)
    }

    /// Arc length of the projection of `p` onto the lane.
    #[inline]
    pub fn arc_of(&self, p: Vec2) -> f32 {
        let d = self.end - self.start;
        let l2 = d.len_sq();
        if l2 < 1e-9 {
            return 0.0;
        }
        clamp((p - self.start).dot(d) / l2, 0.0, 1.0) * self.length
    }

    /// Sideways distance from the lane centre line.
    #[inline]
    pub fn lateral(&self, p: Vec2) -> f32 {
        (p - self.point_at(self.arc_of(p))).len()
    }

    /// `true` when `p` is on (or very near) this lane.
    #[inline]
    pub fn near(&self, p: Vec2, tol: f32) -> bool {
        self.lateral(p) <= tol
    }
}

/// A continuation lane with a steering weight.
#[derive(Clone, Copy, Debug)]
pub struct LaneTarget {
    /// Target lane id.
    pub lane: usize,
    /// Relative pick weight — going straight is preferred over turning.
    pub weight: f32,
}

/// A grid node where two carriageways cross.
#[derive(Clone, Debug)]
pub struct Intersection {
    /// Id inside [`crate::City::intersections`].
    pub id: usize,
    /// Grid coordinates `[line_x, line_z]`.
    pub cell: [usize; 2],
    /// Centre of the junction.
    pub center: Vec2,
    /// Half extent of the junction box.
    pub half: f32,
    /// Lanes arriving into this node.
    pub arrivals: Vec<usize>,
    /// Lanes departing from this node.
    pub departures: Vec<usize>,
    /// Time offset in seconds — staggers the green wave along an avenue.
    pub phase: f32,
}

impl Intersection {
    /// Light cycle length in seconds.
    pub const CYCLE: f32 = 24.0;
    /// Green window per axis in seconds.
    pub const GREEN: f32 = 9.0;

    /// `true` when `p` is inside the junction box.
    #[inline]
    pub fn covers(&self, p: Vec2) -> bool {
        (p.x - self.center.x).abs() <= self.half && (p.y - self.center.y).abs() <= self.half
    }

    /// `true` when `axis` has a green light at simulation time `time`.
    #[inline]
    pub fn light_green(&self, axis: Axis, time: f32) -> bool {
        axis.has_green(time, self.phase, Self::CYCLE, Self::GREEN)
    }
}

/// A zebra crossing in the middle of a carriageway face.
#[derive(Clone, Debug)]
pub struct Crossing {
    /// Id inside [`crate::City::crossings`].
    pub id: usize,
    /// Road that is crossed.
    pub road: usize,
    /// Crossing centre.
    pub center: Vec2,
    /// Walking direction (unit), perpendicular to the carriageway.
    pub dir: Vec2,
    /// Length of the walking path across the tarmac.
    pub length: f32,
    /// Width of the painted band.
    pub width: f32,
}

/// Node id from grid line coordinates.
#[inline]
pub fn node_id(params: &CityParams, i: usize, j: usize) -> usize {
    i * params.road_lines_z() + j
}

/// Grid coordinates of a node id.
#[inline]
pub fn node_cell(params: &CityParams, id: usize) -> [usize; 2] {
    let lz = params.road_lines_z().max(1);
    [id / lz, id % lz]
}

/// World position of a node id (`None` when out of range).
#[inline]
pub fn node_pos(params: &CityParams, id: usize) -> Option<Vec2> {
    if id >= params.node_count() {
        return None;
    }
    let c = node_cell(params, id);
    Some(Vec2::new(
        params.road_center(c[0]),
        params.road_center(c[1]),
    ))
}

/// Build all carriageways and lanes.
pub fn build_roads(params: &CityParams, roads: &mut Vec<Road>, lanes: &mut Vec<Lane>) {
    let half = params.road_width * 0.5;
    let off = params.road_width * 0.25;

    // (axis of travel, grid lines the carriageways sit on, faces per line).
    // Travel along Z happens on the `road_lines_x` lines: node `k` is `node(line, k)`
    // so a line holds `blocks_z` faces. Travel along X is the mirror image.
    let plan: [(Axis, usize, usize); 2] = [
        (Axis::NorthSouth, params.road_lines_x(), params.blocks_z),
        (Axis::EastWest, params.road_lines_z(), params.blocks_x),
    ];

    for &(axis, lines, faces) in plan.iter() {
        for line in 0..lines {
            let at = params.road_center(line);
            let kind = kind_for(line, params.major_period);
            for k in 0..faces {
                let (a, b) = match axis {
                    // Runs along X: the varying coordinate is the X line index.
                    Axis::EastWest => (node_id(params, k, line), node_id(params, k + 1, line)),
                    // Runs along Z: the varying coordinate is the second index.
                    Axis::NorthSouth => (node_id(params, line, k), node_id(params, line, k + 1)),
                };
                // A face must stay on its own grid line: `k + 1` may not run past the
                // last line of the grid, or the node id would wrap onto another line.
                debug_assert!(a < params.node_count() && b < params.node_count());
                if a >= params.node_count() || b >= params.node_count() {
                    continue;
                }
                let start = node_pos(params, a).unwrap_or(Vec2::ZERO);
                let end = node_pos(params, b).unwrap_or(Vec2::ZERO);
                let road_id = roads.len();
                let lane_id = lanes.len();
                let fwd = axis.positive();
                // Right-hand traffic: the lane for travel direction `d` sits at
                // `perp(d)` from the centre line.
                let right = fwd.perp() * off;
                lanes.push(make_lane(
                    lanes.len(),
                    road_id,
                    fwd,
                    start + right + fwd * half,
                    end + right - fwd * half,
                    kind,
                    [a, b],
                ));
                lanes.push(make_lane(
                    lanes.len(),
                    road_id,
                    -fwd,
                    end - right - fwd * half,
                    start - right + fwd * half,
                    kind,
                    [b, a],
                ));
                roads.push(Road {
                    id: road_id,
                    axis,
                    line,
                    at,
                    from_node: a,
                    to_node: b,
                    half_width: half,
                    kind,
                    lanes: [lane_id, lane_id + 1],
                });
            }
        }
    }
}

fn kind_for(line: usize, major: usize) -> RoadKind {
    if major > 1 && line.is_multiple_of(major) {
        RoadKind::Avenue
    } else {
        RoadKind::Street
    }
}

fn make_lane(
    id: usize,
    road: usize,
    dir: Vec2,
    start: Vec2,
    end: Vec2,
    kind: RoadKind,
    nodes: [usize; 2],
) -> Lane {
    Lane {
        id,
        road,
        dir,
        start,
        end,
        length: (end - start).len(),
        speed_limit: match kind {
            RoadKind::Street => 8.0,
            RoadKind::Avenue => 11.5,
        },
        nodes,
        next: Vec::new(),
    }
}

/// Place an [`Intersection`] on every grid node.
pub fn build_intersections(params: &CityParams, out: &mut Vec<Intersection>) {
    for i in 0..params.road_lines_x() {
        for j in 0..params.road_lines_z() {
            let id = out.len();
            out.push(Intersection {
                id,
                cell: [i, j],
                center: Vec2::new(params.road_center(i), params.road_center(j)),
                half: params.road_width * 0.5,
                arrivals: Vec::new(),
                departures: Vec::new(),
                // Green wave: shift the phase once per block travelled.
                phase: (((i + j) / params.major_period.max(1)) as f32) * 3.0,
            });
        }
    }
}

/// Fill `arrivals` / `departures` of every junction and `next` of every lane.
pub fn connect_lanes(lanes: &mut [Lane], intersections: &mut [Intersection]) {
    let count = intersections.len().max(1);
    let mut arrivals: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut departures: Vec<Vec<usize>> = vec![Vec::new(); count];
    for lane in lanes.iter() {
        if lane.nodes[0] < count && lane.nodes[1] < count {
            departures[lane.nodes[0]].push(lane.id);
            arrivals[lane.nodes[1]].push(lane.id);
        }
    }
    for it in intersections.iter_mut() {
        if it.id < count {
            it.arrivals = std::mem::take(&mut arrivals[it.id]);
            it.departures = std::mem::take(&mut departures[it.id]);
        }
    }
    // `dir` is copied out first: `lanes` is mutably borrowed by the outer loop, so it
    // cannot be indexed while `lane` is alive.
    let dirs: Vec<Vec2> = lanes.iter().map(|l| l.dir).collect();
    for lane in lanes.iter_mut() {
        let node = lane.nodes[1];
        let mut targets = Vec::new();
        if let Some(list) = intersections.get(node) {
            for &cand in &list.departures {
                if cand == lane.id {
                    continue;
                }
                let alignment = dirs[cand].dot(lane.dir);
                // straight ≈ 1 · turn ≈ 0 · U-turn ≈ -1 (legal but very unlikely)
                let weight = if alignment > 0.98 {
                    6.0
                } else if alignment < -0.98 {
                    0.05
                } else {
                    1.0
                };
                targets.push(LaneTarget { lane: cand, weight });
            }
        }
        lane.next = targets;
    }
}

/// One zebra crossing in the middle of every carriageway face.
pub fn build_crossings(params: &CityParams, roads: &[Road], out: &mut Vec<Crossing>) {
    for r in roads {
        let mid = (node_pos(params, r.from_node).unwrap_or(Vec2::ZERO)
            + node_pos(params, r.to_node).unwrap_or(Vec2::ZERO))
            * 0.5;
        out.push(Crossing {
            id: out.len(),
            road: r.id,
            center: r.axis.point(r.axis.along(mid), r.at),
            dir: r.axis.positive().perp(),
            length: r.half_width * 2.0,
            width: 2.6,
        });
    }
}
