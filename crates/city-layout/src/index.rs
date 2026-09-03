//! Broad-phase spatial hash over solid obstacles.
//!
//! A uniform grid of [`CELL_SIZE`] metre cells holds the ids of the [`IndexItem`]s
//! that overlap it. Collision resolution and camera occlusion only ever look at a
//! handful of cells, which keeps per-tick collision resolution cheap.

use city_math::{hash::world_to_cell, Aabb2, Vec2};

/// Cell size of the broad-phase grid (metres).
pub const CELL_SIZE: f32 = 12.0;

/// What an [`IndexItem`] describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexKind {
    /// A building footprint.
    Building,
    /// A street prop that blocks movement (bus shelter, planter, barrier …).
    Prop,
}

/// One solid entry of the index.
#[derive(Clone, Debug)]
pub struct IndexItem {
    /// Stable id (index into whichever collection created it).
    pub id: usize,
    /// Category tag.
    pub kind: IndexKind,
    /// Solid footprint on the ground plane.
    pub solid: Aabb2,
    /// Height in metres (0 for kerbs and bollards; used for camera occlusion).
    pub height: f32,
}

/// Uniform spatial hash of solid footprints.
#[derive(Clone, Debug)]
pub struct SpatialIndex {
    cell: f32,
    buckets: Vec<Vec<usize>>,
    items: Vec<IndexItem>,
    origin: Vec2,
    dim: [usize; 2],
}

impl SpatialIndex {
    /// Empty index covering `bounds`.
    pub fn new(cell: f32, bounds: Aabb2) -> SpatialIndex {
        let cell = cell.max(1.0);
        let size = bounds.size();
        let nx = (size.x / cell).ceil().max(0.0) as usize + 2;
        let nz = (size.y / cell).ceil().max(0.0) as usize + 2;
        SpatialIndex {
            cell,
            buckets: vec![Vec::new(); nx * nz],
            items: Vec::new(),
            origin: bounds.min - Vec2::new(cell, cell),
            dim: [nx, nz],
        }
    }

    /// Number of indexed items.
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` when nothing was inserted.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Cell size in metres.
    #[inline]
    pub fn cell_size(&self) -> f32 {
        self.cell
    }

    /// Grid dimensions in cells (diagnostics).
    #[inline]
    pub fn grid_dims(&self) -> [usize; 2] {
        self.dim
    }

    /// All items (unordered).
    #[inline]
    pub fn items(&self) -> &[IndexItem] {
        &self.items
    }

    /// Item by id.
    #[inline]
    pub fn item(&self, id: usize) -> Option<&IndexItem> {
        self.items.get(id)
    }

    fn cell_range(&self, b: Aabb2) -> (i32, i32, i32, i32) {
        let lo = b.min - self.origin;
        let hi = b.max - self.origin;
        (
            world_to_cell(lo.x, self.cell),
            world_to_cell(lo.y, self.cell),
            world_to_cell(hi.x, self.cell),
            world_to_cell(hi.y, self.cell),
        )
    }

    fn bucket(&self, cx: i32, cz: i32) -> Option<&Vec<usize>> {
        if cx < 0 || cz < 0 {
            return None;
        }
        let (cx, cz) = (cx as usize, cz as usize);
        if cx >= self.dim[0] || cz >= self.dim[1] {
            return None;
        }
        self.buckets.get(cx * self.dim[1] + cz)
    }

    fn bucket_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Vec<usize>> {
        if cx < 0 || cz < 0 {
            return None;
        }
        let (cx, cz) = (cx as usize, cz as usize);
        if cx >= self.dim[0] || cz >= self.dim[1] {
            return None;
        }
        self.buckets.get_mut(cx * self.dim[1] + cz)
    }

    /// Insert an item, registering it in every cell its footprint touches.
    pub fn insert(&mut self, item: IndexItem) {
        let id = self.items.len();
        let footprint = item.solid;
        self.items.push(item);
        let (x0, z0, x1, z1) = self.cell_range(footprint);
        for cx in x0..=x1 {
            for cz in z0..=z1 {
                if let Some(bucket) = self.bucket_mut(cx, cz) {
                    if !bucket_contains(bucket, id) {
                        bucket.push(id);
                    }
                }
            }
        }
    }

    /// Ids whose cell overlaps the circle `(p, radius)`, de-duplicated.
    pub fn candidates(&self, p: Vec2, radius: f32) -> Vec<usize> {
        let b = Aabb2::from_center_size(p, Vec2::new(radius.max(0.0), radius.max(0.0)));
        let (x0, z0, x1, z1) = self.cell_range(b);
        let mut out: Vec<usize> = Vec::new();
        for cx in x0..=x1 {
            for cz in z0..=z1 {
                if let Some(bucket) = self.bucket(cx, cz) {
                    for &id in bucket {
                        if !out.contains(&id) {
                            out.push(id);
                        }
                    }
                }
            }
        }
        out
    }

    /// `true` when a circle of `radius` at `p` overlaps a solid footprint.
    pub fn overlaps_circle(&self, p: Vec2, radius: f32) -> bool {
        self.candidates(p, radius)
            .iter()
            .any(|&id| self.items[id].solid.signed_distance(p) < radius.max(0.0))
    }

    /// `true` when `p` is strictly inside a solid footprint.
    pub fn contains_point(&self, p: Vec2) -> bool {
        self.candidates(p, 0.05)
            .iter()
            .any(|&id| self.items[id].solid.contains(p))
    }

    /// Height of the tallest solid at `p` (0 on open ground).
    pub fn height_at(&self, p: Vec2) -> f32 {
        let mut h = 0.0f32;
        for id in self.candidates(p, 0.1) {
            let it = &self.items[id];
            if it.solid.contains(p) && it.height > h {
                h = it.height;
            }
        }
        h
    }

    /// Nearest solid within `max_dist`, as `(distance, item)`.
    pub fn nearest(&self, p: Vec2, max_dist: f32) -> Option<(f32, &IndexItem)> {
        let mut best: Option<(f32, usize)> = None;
        for id in self.candidates(p, max_dist + self.cell) {
            let d = self.items[id].solid.signed_distance(p).abs();
            if d <= max_dist && best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, id));
            }
        }
        best.map(|(d, id)| (d, &self.items[id]))
    }

    /// Deterministic fingerprint, used to prove two runs built the same index.
    pub fn checksum(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for it in &self.items {
            h = mix_into(h, it.solid.min.x.to_bits());
            h = mix_into(h, it.solid.max.y.to_bits());
            h = mix_into(h, (it.height * 100.0) as u32);
        }
        mix_into(h, self.buckets.len() as u32)
    }
}

/// FNV-1a style mixing step.
fn mix_into(mut h: u64, v: u32) -> u64 {
    h ^= v as u64;
    h = h.wrapping_mul(0x100_0000_01b3);
    (h >> 7) | (h << (64 - 7))
}

/// `true` when `bucket` already holds `id`.
fn bucket_contains(bucket: &[usize], id: usize) -> bool {
    bucket.contains(&id)
}
