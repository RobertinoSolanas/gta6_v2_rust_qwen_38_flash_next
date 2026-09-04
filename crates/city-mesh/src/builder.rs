//! Append-only mesh builder: position(3) + normal(3) + colour(3), array of structs.
//!
//! This is the vertex format the live GL path uploads verbatim, so a builder run and a
//! GPU buffer hold exactly the same floats.

/// Floats per vertex: position(3) + normal(3) + colour(3).
pub const FLOATS_PER_VERTEX: usize = 9;

/// Number of vertices in a raw vertex buffer of this format.
#[inline]
pub fn vertex_count(verts: &[f32]) -> usize {
    verts.len() / FLOATS_PER_VERTEX
}

/// Append-only mesh builder.
#[derive(Clone, Debug, Default)]
pub struct MeshBuilder {
    pub verts: Vec<f32>,
}

impl MeshBuilder {
    pub fn new() -> MeshBuilder {
        MeshBuilder { verts: Vec::new() }
    }

    /// Number of vertices written so far.
    #[inline]
    pub fn len(&self) -> usize {
        self.verts.len() / FLOATS_PER_VERTEX
    }

    /// `true` when nothing was written.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.verts.is_empty()
    }

    /// Number of triangles written (each quad is two triangles, each triangle 3 verts).
    #[inline]
    pub fn triangles(&self) -> usize {
        self.len() / 3
    }

    /// Raw byte length of the buffer as uploaded (`len * FLOATS_PER_VERTEX * 4`).
    #[inline]
    pub fn byte_len(&self) -> usize {
        self.verts.len() * 4
    }

    /// Append one vertex.
    #[inline]
    pub fn vert(&mut self, p: [f32; 3], n: [f32; 3], c: [f32; 3]) {
        self.verts.extend_from_slice(&p);
        self.verts.extend_from_slice(&n);
        self.verts.extend_from_slice(&c);
    }

    /// Read vertex `i` back as (position, normal, colour).
    pub fn get(&self, i: usize) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let s = i * FLOATS_PER_VERTEX;
        (
            [self.verts[s], self.verts[s + 1], self.verts[s + 2]],
            [self.verts[s + 3], self.verts[s + 4], self.verts[s + 5]],
            [self.verts[s + 6], self.verts[s + 7], self.verts[s + 8]],
        )
    }

    /// Two triangles forming a quad `a-b-c-d` (counter-clockwise seen from `n`).
    pub fn quad(
        &mut self,
        a: [f32; 3],
        b: [f32; 3],
        c: [f32; 3],
        d: [f32; 3],
        n: [f32; 3],
        col: [f32; 3],
    ) {
        self.vert(a, n, col);
        self.vert(b, n, col);
        self.vert(c, n, col);
        self.vert(a, n, col);
        self.vert(c, n, col);
        self.vert(d, n, col);
    }

    /// Axis-aligned box: top face in `top`, the five other faces in `wall`.
    pub fn box_shaded(&mut self, min: [f32; 3], max: [f32; 3], top: [f32; 3], wall: [f32; 3]) {
        let (x0, y0, z0) = (min[0], min[1], min[2]);
        let (x1, y1, z1) = (max[0], max[1], max[2]);
        // top
        self.quad(
            [x0, y1, z0],
            [x0, y1, z1],
            [x1, y1, z1],
            [x1, y1, z0],
            [0.0, 1.0, 0.0],
            top,
        );
        // bottom (keeps the shape closed)
        self.quad(
            [x0, y0, z1],
            [x0, y0, z0],
            [x1, y0, z0],
            [x1, y0, z1],
            [0.0, -1.0, 0.0],
            wall,
        );
        // four walls
        self.quad(
            [x0, y0, z1],
            [x1, y0, z1],
            [x1, y1, z1],
            [x0, y1, z1],
            [0.0, 0.0, 1.0],
            wall,
        );
        self.quad(
            [x1, y0, z0],
            [x0, y0, z0],
            [x0, y1, z0],
            [x1, y1, z0],
            [0.0, 0.0, -1.0],
            wall,
        );
        self.quad(
            [x1, y0, z1],
            [x1, y0, z0],
            [x1, y1, z0],
            [x1, y1, z1],
            [1.0, 0.0, 0.0],
            wall,
        );
        self.quad(
            [x0, y0, z0],
            [x0, y0, z1],
            [x0, y1, z1],
            [x0, y1, z0],
            [-1.0, 0.0, 0.0],
            wall,
        );
    }

    /// Flat ground quad at `y`.
    pub fn ground(&mut self, min: [f32; 2], max: [f32; 2], y: f32, col: [f32; 3]) {
        let n = [0.0, 1.0, 0.0];
        self.quad(
            [min[0], y, min[1]],
            [min[0], y, max[1]],
            [max[0], y, max[1]],
            [max[0], y, min[1]],
            n,
            col,
        );
    }

    /// Box centred on `center` (XZ), half-extents `hx`/`hz`, spanning `y0..=y1`,
    /// rotated `yaw` radians about the world Y (`yaw = 0` ⇒ axis-aligned, `+X` = front).
    ///
    /// The four side normals are rotated with the box; the caps keep `±Y`.
    #[allow(clippy::too_many_arguments)] // eight geometry arguments read better than a struct
    pub fn box_yaw(
        &mut self,
        center: [f32; 2],
        hx: f32,
        hz: f32,
        y0: f32,
        y1: f32,
        yaw: f32,
        top: [f32; 3],
        wall: [f32; 3],
    ) {
        let (c, s) = (yaw.cos(), yaw.sin());
        // corner (sx, sz) of the footprint, in world XZ
        let corner = |sx: f32, sz: f32| -> [f32; 2] {
            let (lx, lz) = (sx * hx, sz * hz);
            [center[0] + c * lx - s * lz, center[1] + s * lx + c * lz]
        };
        let n = |nx: f32, nz: f32| -> [f32; 3] { [c * nx - s * nz, 0.0, s * nx + c * nz] };

        let a = corner(-1.0, -1.0); // -X -Z
        let b = corner(1.0, -1.0); // +X -Z
        let cc = corner(1.0, 1.0); // +X +Z
        let d = corner(-1.0, 1.0); // -X +Z
        let lo = |p: [f32; 2]| [p[0], y0, p[1]];
        let hi = |p: [f32; 2]| [p[0], y1, p[1]];

        // caps
        self.quad(hi(a), hi(d), hi(cc), hi(b), [0.0, 1.0, 0.0], top);
        self.quad(lo(a), lo(b), lo(cc), lo(d), [0.0, -1.0, 0.0], wall);
        // sides: -Z, +X, +Z, -X (each listed so its normal faces outward)
        self.quad(lo(a), lo(b), hi(b), hi(a), n(0.0, -1.0), wall);
        self.quad(lo(b), lo(cc), hi(cc), hi(b), n(1.0, 0.0), wall);
        self.quad(lo(cc), lo(d), hi(d), hi(cc), n(0.0, 1.0), wall);
        self.quad(lo(d), lo(a), hi(a), hi(d), n(-1.0, 0.0), wall);
    }

    /// Borrow the raw buffer (the exact floats an upload would take).
    #[inline]
    pub fn as_slice(&self) -> &[f32] {
        &self.verts
    }

    pub fn into_vec(self) -> Vec<f32> {
        self.verts
    }
}
