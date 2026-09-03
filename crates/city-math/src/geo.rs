//! Axis aligned boxes, segments and rays (ground plane + 3D volumes).

use crate::vec::{Vec2, Vec3};

/// Axis aligned bounding box on the ground plane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Aabb2 {
    pub min: Vec2,
    pub max: Vec2,
}

impl Aabb2 {
    /// Inverted/empty box.
    pub const EMPTY: Aabb2 = Aabb2 {
        min: Vec2::new(f32::MAX, f32::MAX),
        max: Vec2::new(f32::MIN, f32::MIN),
    };

    #[inline]
    pub fn new(min: Vec2, max: Vec2) -> Aabb2 {
        Aabb2 { min, max }
    }

    #[inline]
    pub fn from_center_size(center: Vec2, half: Vec2) -> Aabb2 {
        Aabb2 {
            min: Vec2::new(center.x - half.x, center.y - half.y),
            max: Vec2::new(center.x + half.x, center.y + half.y),
        }
    }

    #[inline]
    pub fn from_min_size(min: Vec2, size: Vec2) -> Aabb2 {
        Aabb2 {
            min,
            max: min + size,
        }
    }

    #[inline]
    pub fn center(self) -> Vec2 {
        Vec2::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }

    #[inline]
    pub fn size(self) -> Vec2 {
        Vec2::new(self.max.x - self.min.x, self.max.y - self.min.y)
    }

    #[inline]
    pub fn area(self) -> f32 {
        (self.max.x - self.min.x).max(0.0) * (self.max.y - self.min.y).max(0.0)
    }

    #[inline]
    pub fn contains(self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    #[inline]
    pub fn contains_padded(self, p: Vec2, pad: f32) -> bool {
        self.grown(pad).contains(p)
    }

    #[inline]
    pub fn intersects(self, o: Aabb2) -> bool {
        self.min.x <= o.max.x
            && self.max.x >= o.min.x
            && self.min.y <= o.max.y
            && self.max.y >= o.min.y
    }

    #[inline]
    pub fn expand(self, p: Vec2) -> Aabb2 {
        Aabb2 {
            min: self.min.min(p),
            max: self.max.max(p),
        }
    }

    #[inline]
    pub fn grown(self, pad: f32) -> Aabb2 {
        Aabb2 {
            min: Vec2::new(self.min.x - pad, self.min.y - pad),
            max: Vec2::new(self.max.x + pad, self.max.y + pad),
        }
    }

    /// Clamp a point into the box.
    #[inline]
    pub fn closest_point(self, p: Vec2) -> Vec2 {
        Vec2::new(
            crate::clamp(p.x, self.min.x, self.max.x),
            crate::clamp(p.y, self.min.y, self.max.y),
        )
    }

    /// Signed distance to the surface (negative when inside).
    #[inline]
    pub fn signed_distance(self, p: Vec2) -> f32 {
        let dx = (self.min.x - p.x).max(p.x - self.max.x);
        let dy = (self.min.y - p.y).max(p.y - self.max.y);
        if dx < 0.0 && dy < 0.0 {
            // Inside: negative, closest to the closest face.
            dx.max(dy)
        } else {
            Vec2::new(dx.max(0.0), dy.max(0.0)).len()
        }
    }

    /// Resolve a moving circle of `radius` against this box.
    ///
    /// Returns the corrected position when `p` overlaps the inflated box,
    /// otherwise `None` (nothing to resolve). Uses the shallowest axis, which is
    /// what gives the familiar "slide along a wall" behaviour.
    pub fn push_out(self, p: Vec2, radius: f32) -> Option<Vec2> {
        let r = radius.max(0.0);
        let min = Vec2::new(self.min.x - r, self.min.y - r);
        let max = Vec2::new(self.max.x + r, self.max.y + r);
        if p.x <= min.x || p.x >= max.x || p.y <= min.y || p.y >= max.y {
            return None;
        }
        let dl = p.x - min.x;
        let dr = max.x - p.x;
        let db = p.y - min.y;
        let dt = max.y - p.y;
        let m = dl.min(dr).min(db).min(dt);
        let out = if m == dl {
            Vec2::new(min.x, p.y)
        } else if m == dr {
            Vec2::new(max.x, p.y)
        } else if m == db {
            Vec2::new(p.x, min.y)
        } else {
            Vec2::new(p.x, max.y)
        };
        Some(out)
    }
}

/// Axis aligned box in 3D (buildings, vehicles, props).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Aabb3 {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb3 {
    pub const EMPTY: Aabb3 = Aabb3 {
        min: Vec3::new(f32::MAX, f32::MAX, f32::MAX),
        max: Vec3::new(f32::MIN, f32::MIN, f32::MIN),
    };

    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Aabb3 {
        Aabb3 { min, max }
    }

    #[inline]
    pub fn from_center_size(center: Vec3, size: Vec3) -> Aabb3 {
        let h = size * 0.5;
        Aabb3 {
            min: center - h,
            max: center + h,
        }
    }

    #[inline]
    pub fn center(self) -> Vec3 {
        self.min.lerp(self.max, 0.5)
    }

    #[inline]
    pub fn size(self) -> Vec3 {
        self.max - self.min
    }

    #[inline]
    pub fn contains(self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    #[inline]
    pub fn intersects(self, o: Aabb3) -> bool {
        self.min.x <= o.max.x
            && self.max.x >= o.min.x
            && self.min.y <= o.max.y
            && self.max.y >= o.min.y
            && self.min.z <= o.max.z
            && self.max.z >= o.min.z
    }

    #[inline]
    pub fn expand(self, p: Vec3) -> Aabb3 {
        Aabb3 {
            min: self.min.min(p),
            max: self.max.max(p),
        }
    }

    #[inline]
    pub fn grown(self, pad: f32) -> Aabb3 {
        Aabb3 {
            min: self.min - Vec3::new(pad, pad, pad),
            max: self.max + Vec3::new(pad, pad, pad),
        }
    }

    /// Project onto the ground plane.
    #[inline]
    pub fn footprint(self) -> Aabb2 {
        Aabb2 {
            min: Vec2::new(self.min.x, self.min.z),
            max: Vec2::new(self.max.x, self.max.z),
        }
    }

    /// Slab test. Returns `(t_near, t_far)`; `None` for a miss or a hit behind
    /// the origin.
    pub fn ray(self, origin: Vec3, dir: Vec3) -> Option<(f32, f32)> {
        let o = [origin.x, origin.y, origin.z];
        let d = [dir.x, dir.y, dir.z];
        let bmin = [self.min.x, self.min.y, self.min.z];
        let bmax = [self.max.x, self.max.y, self.max.z];
        let mut tmin = f32::NEG_INFINITY;
        let mut tmax = f32::INFINITY;
        for i in 0..3 {
            if d[i].abs() < 1e-8 {
                if o[i] < bmin[i] || o[i] > bmax[i] {
                    return None;
                }
                continue;
            }
            let inv = 1.0 / d[i];
            let mut t0 = (bmin[i] - o[i]) * inv;
            let mut t1 = (bmax[i] - o[i]) * inv;
            if t0 > t1 {
                core::mem::swap(&mut t0, &mut t1);
            }
            tmin = tmin.max(t0);
            tmax = tmax.min(t1);
            if tmin > tmax {
                return None;
            }
        }
        if tmax < 0.0 {
            None
        } else {
            Some((tmin.max(0.0), tmax))
        }
    }
}

/// 2D segment (lane segments, sight rays, road centrelines).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Seg2 {
    pub a: Vec2,
    pub b: Vec2,
}

impl Seg2 {
    #[inline]
    pub fn new(a: Vec2, b: Vec2) -> Seg2 {
        Seg2 { a, b }
    }
    #[inline]
    pub fn dir(self) -> Vec2 {
        (self.b - self.a).norm()
    }
    #[inline]
    pub fn len(self) -> f32 {
        (self.b - self.a).len()
    }
    /// Closest point on the segment to `p`, plus the parametric `t`.
    pub fn closest(self, p: Vec2) -> (Vec2, f32) {
        let ab = self.b - self.a;
        let l2 = ab.len_sq();
        if l2 < 1e-12 {
            return (self.a, 0.0);
        }
        let t = crate::clamp((p - self.a).dot(ab) / l2, 0.0, 1.0);
        (self.a + ab * t, t)
    }
    #[inline]
    pub fn distance(self, p: Vec2) -> f32 {
        self.closest(p).0.dist(p)
    }
    /// Intersection point of two segments (`None` when parallel/outside).
    pub fn intersect(self, o: Seg2) -> Option<Vec2> {
        let r = self.b - self.a;
        let e = o.b - o.a;
        let denom = r.cross(e);
        if denom.abs() < 1e-8 {
            return None;
        }
        let d = o.a - self.a;
        let t = d.cross(e) / denom;
        let u = d.cross(r) / denom;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            Some(self.a + r * t)
        } else {
            None
        }
    }
}

/// A ray with an explicit max distance (camera occlusion, sight checks).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Ray2 {
    pub origin: Vec2,
    pub dir: Vec2,
    pub max_t: f32,
}

impl Ray2 {
    #[inline]
    pub fn new(origin: Vec2, dir: Vec2, max_t: f32) -> Ray2 {
        Ray2 {
            origin,
            dir: dir.norm(),
            max_t,
        }
    }
    #[inline]
    pub fn at(self, t: f32) -> Vec2 {
        self.origin + self.dir * t
    }
    /// Distance to the first hit against a segment, or `None`.
    pub fn hit_seg(self, s: Seg2) -> Option<f32> {
        let e = s.b - s.a;
        let denom = self.dir.cross(e);
        if denom.abs() < 1e-8 {
            return None;
        }
        let d = s.a - self.origin;
        let t = d.cross(e) / denom;
        let u = d.cross(self.dir) / denom;
        if t >= 0.0 && t <= self.max_t && (0.0..=1.0).contains(&u) {
            Some(t)
        } else {
            None
        }
    }
}
