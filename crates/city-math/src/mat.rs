//! 4x4 matrices, column major (`m[col][row]`) to match WebGL uploads.

use crate::vec::{Vec3, Vec4};
use crate::PI;

/// Column-major 4x4 matrix. `cols[c]` is the image of basis vector `e_c`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

impl Default for Mat4 {
    #[inline]
    fn default() -> Self {
        Mat4::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    pub const ZERO: Mat4 = Mat4 {
        cols: [[0.0; 4]; 4],
    };

    /// Element access `[col][row]`.
    #[inline]
    pub fn at(&self, col: usize, row: usize) -> f32 {
        self.cols[col][row]
    }

    #[inline]
    pub fn set(&mut self, col: usize, row: usize, v: f32) {
        self.cols[col][row] = v;
    }

    /// Build from column major floats (16 values).
    pub fn from_cols(values: [[f32; 4]; 4]) -> Mat4 {
        Mat4 { cols: values }
    }

    /// Translate-only matrix.
    pub fn translation(t: Vec3) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.cols[3] = [t.x, t.y, t.z, 1.0];
        Mat4 { cols: m.cols }
    }

    /// Uniform or per-axis scale.
    pub fn scale(s: Vec3) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.cols[0][0] = s.x;
        m.cols[1][1] = s.y;
        m.cols[2][2] = s.z;
        m
    }

    pub fn scale_uniform(s: f32) -> Mat4 {
        Mat4::scale(Vec3::new(s, s, s))
    }

    /// Rotation about the X axis (rolls around `+X`).
    pub fn rotate_x(a: f32) -> Mat4 {
        let (s, c) = (a.sin(), a.cos());
        Mat4 {
            cols: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, c, s, 0.0],
                [0.0, -s, c, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Rotation about the Y axis (yaw).
    pub fn rotate_y(a: f32) -> Mat4 {
        let (s, c) = (a.sin(), a.cos());
        Mat4 {
            cols: [
                [c, 0.0, -s, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [s, 0.0, c, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Rotation about the Z axis (roll / pitch for the camera rig).
    pub fn rotate_z(a: f32) -> Mat4 {
        let (s, c) = (a.sin(), a.cos());
        Mat4 {
            cols: [
                [c, s, 0.0, 0.0],
                [-s, c, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// TRS transform (`T * Ry * Rz * S`) used for every rendered instance.
    ///
    /// Order matters: the instance is scaled, then pitched, then yawed, then
    /// moved — which keeps the translation in the last column.
    pub fn compose(translation: Vec3, yaw: f32, pitch: f32, scale: Vec3) -> Mat4 {
        Mat4::translation(translation)
            .mul(&Mat4::rotate_y(yaw))
            .mul(&Mat4::rotate_z(pitch))
            .mul(&Mat4::scale(scale))
    }

    /// Matrix product `self * other` (apply `other` first).
    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let mut out = Mat4::ZERO;
        for c in 0..4 {
            for r in 0..4 {
                let mut sum = 0.0f32;
                for k in 0..4 {
                    sum += self.cols[k][r] * o.cols[c][k];
                }
                out.cols[c][r] = sum;
            }
        }
        out
    }

    /// Transform a point (`w = 1`).
    pub fn point(&self, p: Vec3) -> Vec3 {
        let v = self.vec4(Vec4::new(p.x, p.y, p.z, 1.0));
        Vec3::new(v.x, v.y, v.z)
    }

    /// Transform a direction (translation ignored).
    pub fn dir(&self, d: Vec3) -> Vec3 {
        let v = self.vec4(Vec4::new(d.x, d.y, d.z, 0.0));
        Vec3::new(v.x, v.y, v.z)
    }

    pub fn vec4(&self, v: Vec4) -> Vec4 {
        let mut out = [0.0f32; 4];
        for r in 0..4 {
            out[r] = self.cols[0][r] * v.x
                + self.cols[1][r] * v.y
                + self.cols[2][r] * v.z
                + self.cols[3][r] * v.w;
        }
        Vec4::new(out[0], out[1], out[2], out[3])
    }

    /// Right-handed perspective projection (OpenGL style, NDC z in `[-1,1]`).
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fov_y.max(1e-4) / 2.0).tan();
        let nf = 1.0 / (near - far);
        Mat4 {
            cols: [
                [f / aspect.max(1e-4), 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [0.0, 0.0, (far + near) * nf, -1.0],
                [0.0, 0.0, 2.0 * far * near * nf, 0.0],
            ],
        }
    }

    /// Orthographic projection, mainly for the shadow map.
    pub fn ortho(l: f32, r: f32, b: f32, t: f32, n: f32, f: f32) -> Mat4 {
        let (rl, tb, fn_) = (1.0 / (r - l), 1.0 / (t - b), 1.0 / (f - n));
        Mat4 {
            cols: [
                [2.0 * rl, 0.0, 0.0, 0.0],
                [0.0, 2.0 * tb, 0.0, 0.0],
                [0.0, 0.0, -2.0 * fn_, 0.0],
                [-(r + l) * rl, -(t + b) * tb, -(f + n) * fn_, 1.0],
            ],
        }
    }

    /// Camera matrix: eye looking at `center`, `up` defines roll.
    pub fn look_at(eye: Vec3, center: Vec3, up: Vec3) -> Mat4 {
        let f = (center - eye).norm();
        // Guard against looking exactly along `up`.
        let mut u = up;
        let mut s = f.cross(u);
        if s.len_sq() < 1e-8 {
            u = if f.x.abs() > 0.9 { Vec3::X } else { Vec3::Z };
            s = f.cross(u);
        }
        let s = s.norm();
        u = s.cross(f);
        Mat4 {
            cols: [
                [s.x, u.x, -f.x, 0.0],
                [s.y, u.y, -f.y, 0.0],
                [s.z, u.z, -f.z, 0.0],
                [-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0],
            ],
        }
    }

    /// `bias` in NDC-ish units for polygon offset style depth biasing in shaders.
    pub fn bias_xz(&self, dx: f32, dz: f32) -> Mat4 {
        let mut m = *self;
        m.cols[3][0] += dx;
        m.cols[3][2] += dz;
        m
    }

    /// Flatten to 16 column-major floats — the layout `uniformMatrix4fv` expects.
    #[inline]
    pub fn to_flat(&self) -> [f32; 16] {
        let mut out = [0.0f32; 16];
        let mut i = 0;
        for c in 0..4 {
            for r in 0..4 {
                out[i] = self.cols[c][r];
                i += 1;
            }
        }
        out
    }

    /// Row major copy (handy in tests and debugging).
    pub fn row_major(&self) -> [[f32; 4]; 4] {
        let mut r = [[0.0f32; 4]; 4];
        for c in 0..4 {
            for i in 0..4 {
                r[i][c] = self.cols[c][i];
            }
        }
        r
    }

    /// Approximate uniform scale factor of the upper 3x3.
    pub fn avg_scale(&self) -> f32 {
        let sx = Vec3::new(self.cols[0][0], self.cols[0][1], self.cols[0][2]).len();
        let sy = Vec3::new(self.cols[1][0], self.cols[1][1], self.cols[1][2]).len();
        let sz = Vec3::new(self.cols[2][0], self.cols[2][1], self.cols[2][2]).len();
        (sx + sy + sz) / 3.0
    }

    /// A quarter turn, handy for axis aligned test cases.
    pub const QUARTER: f32 = PI / 2.0;
}
