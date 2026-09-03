//! Vector types used across the whole simulation (`Y` is up).

use crate::{clamp, lerp};

/// 2D vector, `x`/`z` on the ground plane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const X: Vec2 = Vec2 { x: 1.0, y: 0.0 };
    pub const Y: Vec2 = Vec2 { x: 0.0, y: 1.0 };

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }
    #[inline]
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    /// 2D cross product (z component of the 3D cross).
    #[inline]
    pub fn cross(self, o: Vec2) -> f32 {
        self.x * o.y - self.y * o.x
    }
    #[inline]
    pub fn len_sq(self) -> f32 {
        self.dot(self)
    }
    #[inline]
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    #[inline]
    pub fn dist_sq(self, o: Vec2) -> f32 {
        (self - o).len_sq()
    }
    #[inline]
    pub fn dist(self, o: Vec2) -> f32 {
        (self - o).len()
    }
    /// Returns [`Vec2::ZERO`] for degenerate vectors instead of `NaN`.
    #[inline]
    pub fn norm(self) -> Vec2 {
        let l = self.len();
        if l > 1e-8 {
            Vec2::new(self.x / l, self.y / l)
        } else {
            Vec2::ZERO
        }
    }
    #[inline]
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }
    #[inline]
    pub fn lerp(self, o: Vec2, t: f32) -> Vec2 {
        Vec2::new(lerp(self.x, o.x, t), lerp(self.y, o.y, t))
    }
    #[inline]
    pub fn clamp_len(self, max: f32) -> Vec2 {
        let l = self.len();
        if l > max && l > 1e-8 {
            self * (max / l)
        } else {
            self
        }
    }
    #[inline]
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
    #[inline]
    pub fn from_angle(a: f32) -> Vec2 {
        Vec2::new(a.cos(), a.sin())
    }
    #[inline]
    pub fn min(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.min(o.x), self.y.min(o.y))
    }
    #[inline]
    pub fn max(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.max(o.x), self.y.max(o.y))
    }
    #[inline]
    pub fn as_array(self) -> [f32; 2] {
        [self.x, self.y]
    }

    /// Rotate by `+90°` steps (`n` may be negative).
    pub fn rot90(self, n: i32) -> Vec2 {
        match ((n % 4) + 4) % 4 {
            1 => Vec2::new(-self.y, self.x),
            2 => Vec2::new(-self.x, -self.y),
            3 => Vec2::new(self.y, -self.x),
            _ => self,
        }
    }
}

impl core::ops::Add for Vec2 {
    type Output = Vec2;
    #[inline]
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl core::ops::Sub for Vec2 {
    type Output = Vec2;
    #[inline]
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl core::ops::Mul<f32> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}
impl core::ops::Neg for Vec2 {
    type Output = Vec2;
    #[inline]
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}
impl core::ops::AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}

/// 3D vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const UP: Vec3 = Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    pub const X: Vec3 = Vec3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    pub const Z: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }
    /// Build from a ground position plus a height.
    #[inline]
    pub const fn from_xz(p: Vec2, y: f32) -> Vec3 {
        Vec3 {
            x: p.x,
            y,
            z: p.y,
        }
    }
    /// Project onto the ground plane (drops `y`).
    #[inline]
    pub const fn xz(self) -> Vec2 {
        Vec2 { x: self.x, y: self.z }
    }
    #[inline]
    pub fn with_y(self, y: f32) -> Vec3 {
        Vec3::new(self.x, y, self.z)
    }
    #[inline]
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    #[inline]
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    #[inline]
    pub fn len_sq(self) -> f32 {
        self.dot(self)
    }
    #[inline]
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    #[inline]
    pub fn dist(self, o: Vec3) -> f32 {
        (self - o).len()
    }
    #[inline]
    pub fn norm(self) -> Vec3 {
        let l = self.len();
        if l > 1e-8 {
            self * (1.0 / l)
        } else {
            Vec3::ZERO
        }
    }
    #[inline]
    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        Vec3::new(
            lerp(self.x, o.x, t),
            lerp(self.y, o.y, t),
            lerp(self.z, o.z, t),
        )
    }
    #[inline]
    pub fn clamp_len(self, max: f32) -> Vec3 {
        let l = self.len();
        if l > max && l > 1e-8 {
            self * (max / l)
        } else {
            self
        }
    }
    /// Horizontal heading as a yaw angle (`atan2(x, z)`), `0` = `+Z`.
    #[inline]
    pub fn yaw(self) -> f32 {
        self.x.atan2(self.z)
    }
    /// Unit direction from a yaw/pitch pair (same convention as the camera).
    pub fn from_yaw_pitch(yaw: f32, pitch: f32) -> Vec3 {
        let cp = pitch.cos();
        Vec3::new(yaw.sin() * cp, pitch.sin(), yaw.cos() * cp)
    }
    #[inline]
    pub fn min(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.x.min(o.x),
            self.y.min(o.y),
            self.z.min(o.z),
        )
    }
    #[inline]
    pub fn max(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.x.max(o.x),
            self.y.max(o.y),
            self.z.max(o.z),
        )
    }
    #[inline]
    pub fn as_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

impl core::ops::Add for Vec3 {
    type Output = Vec3;
    #[inline]
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl core::ops::Sub for Vec3 {
    type Output = Vec3;
    #[inline]
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl core::ops::Mul<f32> for Vec3 {
    type Output = Vec3;
    #[inline]
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl core::ops::Add<Vec3> for &Vec3 {
    type Output = Vec3;
    #[inline]
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl core::ops::Neg for Vec3 {
    type Output = Vec3;
    #[inline]
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}
impl core::ops::AddAssign for Vec3 {
    #[inline]
    fn add_assign(&mut self, o: Vec3) {
        self.x += o.x;
        self.y += o.y;
        self.z += o.z;
    }
}
impl core::ops::MulAssign<f32> for Vec3 {
    #[inline]
    fn mul_assign(&mut self, s: f32) {
        self.x *= s;
        self.y *= s;
        self.z *= s;
    }
}

/// 4 component vector — homogeneous positions and RGBA colours.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub const ONE: Vec4 = Vec4 {
        x: 1.0,
        y: 1.0,
        z: 1.0,
        w: 1.0,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Vec4 { x, y, z, w }
    }
    #[inline]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Vec4 {
        Vec4 {
            x: r,
            y: g,
            z: b,
            w: 1.0,
        }
    }
    #[inline]
    pub fn mul_rgb(self, s: f32) -> Vec4 {
        Vec4::new(self.x * s, self.y * s, self.z * s, self.w)
    }
    #[inline]
    pub fn lerp(self, o: Vec4, t: f32) -> Vec4 {
        Vec4::new(
            lerp(self.x, o.x, t),
            lerp(self.y, o.y, t),
            lerp(self.z, o.z, t),
            lerp(self.w, o.w, t),
        )
    }
    /// Multiply a colour by scalar light and clamp to `[0,1]` (LDR output).
    #[inline]
    pub fn lit(self, light: f32) -> Vec4 {
        Vec4::new(
            clamp(self.x * light, 0.0, 1.0),
            clamp(self.y * light, 0.0, 1.0),
            clamp(self.z * light, 0.0, 1.0),
            self.w,
        )
    }
    #[inline]
    pub fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
    /// Relative luminance (Rec. 709).
    #[inline]
    pub fn luminance(self) -> f32 {
        0.2126 * self.x + 0.7152 * self.y + 0.0722 * self.z
    }
}

impl core::ops::Add for Vec4 {
    type Output = Vec4;
    #[inline]
    fn add(self, o: Vec4) -> Vec4 {
        Vec4::new(self.x + o.x, self.y + o.y, self.z + o.z, self.w + o.w)
    }
}
impl core::ops::Mul<f32> for Vec4 {
    type Output = Vec4;
    #[inline]
    fn mul(self, s: f32) -> Vec4 {
        Vec4::new(self.x * s, self.y * s, self.z * s, self.w)
    }
}
