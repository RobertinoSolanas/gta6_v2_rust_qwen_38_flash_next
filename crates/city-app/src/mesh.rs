//! CPU geometry builders: everything the renderer needs, from the generated city.

use city_layout::{City, PropKind};

/// Vertex = position(3) + normal(3) + colour(3).
pub const FLOATS_PER_VERTEX: usize = 9;

/// Append-only mesh builder (array of structs, uploaded verbatim).
pub struct MeshBuilder {
    pub verts: Vec<f32>,
}

impl MeshBuilder {
    pub fn new() -> MeshBuilder {
        MeshBuilder { verts: Vec::new() }
    }

    #[inline]
    pub fn vert(&mut self, p: [f32; 3], n: [f32; 3], c: [f32; 3]) {
        self.verts.extend_from_slice(&p);
        self.verts.extend_from_slice(&n);
        self.verts.extend_from_slice(&c);
    }

    /// Two triangles forming a quad `a-b-c-d` (counter-clockwise).
    pub fn quad(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], n: [f32; 3], col: [f32; 3]) {
        self.vert(a, n, col);
        self.vert(b, n, col);
        self.vert(c, n, col);
        self.vert(a, n, col);
        self.vert(c, n, col);
        self.vert(d, n, col);
    }

    /// Axis-aligned box: roof in `top`, sides in `wall`.
    pub fn box_shaded(&mut self, min: [f32; 3], max: [f32; 3], top: [f32; 3], wall: [f32; 3]) {
        let (x0, y0, z0) = (min[0], min[1], min[2]);
        let (x1, y1, z1) = (max[0], max[1], max[2]);
        // top
        self.quad([x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [0.0, 1.0, 0.0], top);
        // bottom (mostly invisible, keeps the shape closed)
        self.quad([x0, y0, z1], [x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [0.0, -1.0, 0.0], wall);
        // four walls
        self.quad([x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1], [0.0, 0.0, 1.0], wall);
        self.quad([x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [0.0, 0.0, -1.0], wall);
        self.quad([x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [1.0, 0.0, 0.0], wall);
        self.quad([x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0], [-1.0, 0.0, 0.0], wall);
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

    pub fn into_vec(self) -> Vec<f32> {
        self.verts
    }
}

impl Default for MeshBuilder {
    fn default() -> Self {
        MeshBuilder::new()
    }
}

/// Colours of the flat-shaded city (no textures anywhere).
pub mod palette {
    pub const ASPHALT: [f32; 3] = [0.17, 0.18, 0.21];
    pub const SIDEWALK: [f32; 3] = [0.34, 0.35, 0.38];
    pub const PARK: [f32; 3] = [0.14, 0.36, 0.17];
    pub const PLAZA: [f32; 3] = [0.42, 0.40, 0.38];
    pub const LOT: [f32; 3] = [0.24, 0.27, 0.27];
    pub const ROOF: [f32; 3] = [0.36, 0.37, 0.41];
    pub const TRUNK: [f32; 3] = [0.27, 0.19, 0.12];
    pub const LEAF: [f32; 3] = [0.16, 0.42, 0.19];
    pub const METAL: [f32; 3] = [0.44, 0.46, 0.50];
    pub const LAMP_ON: [f32; 3] = [1.00, 0.85, 0.52];
    pub const MONUMENT: [f32; 3] = [0.60, 0.58, 0.66];
    pub const CONCRETE: [f32; 3] = [0.40, 0.41, 0.45];
}

/// Facade colour derived from the building's procedural variant.
pub fn facade_color(building: &city_layout::Building) -> [f32; 3] {
    const PALETTE: [[f32; 3]; 6] = [
        [0.56, 0.45, 0.37],
        [0.63, 0.60, 0.55],
        [0.42, 0.52, 0.62],
        [0.68, 0.52, 0.44],
        [0.47, 0.49, 0.56],
        [0.58, 0.58, 0.54],
    ];
    let c = PALETTE[(building.variant as usize) % PALETTE.len()];
    if building.landmark {
        [c[0] * 1.05, c[1] * 1.02, c[2] * 1.1]
    } else {
        c
    }
}

/// Build every static mesh of the city into `m`.
pub fn build_city(city: &City, m: &mut MeshBuilder) {
    let b = city.bounds();

    // base plane: everything is tarmac, then blocks paint their own surface
    m.ground([b.min.x, b.min.y], [b.max.x, b.max.y], 0.0, palette::ASPHALT);

    for block in city.blocks() {
        let col = match block.kind {
            city_layout::BlockKind::Park => palette::PARK,
            city_layout::BlockKind::Plaza => palette::PLAZA,
            city_layout::BlockKind::Lot => palette::LOT,
            city_layout::BlockKind::Urban => palette::SIDEWALK,
        };
        let l = block.lots;
        // sidewalks / block surface, raised 15 cm so blocks read as blocks
        m.ground([l.min.x, l.min.y], [l.max.x, l.max.y], 0.15, col);
        // kerb band around the block
        m.box_shaded(
            [block.bounds.min.x, 0.0, block.bounds.min.y],
            [block.bounds.max.x, 0.15, block.bounds.max.y],
            col,
            palette::CONCRETE,
        );
    }

    // buildings
    for building in city.buildings() {
        let f = &building.footprint;
        let wall = facade_color(building);
        m.box_shaded(
            [f.min.x, 0.0, f.min.y],
            [f.max.x, building.height, f.max.y],
            palette::ROOF,
            wall,
        );
        if building.setback_height > 0.2 {
            let inset_x = f.size().x * (1.0 - building.setback_scale) * 0.5;
            let inset_z = f.size().y * (1.0 - building.setback_scale) * 0.5;
            m.box_shaded(
                [f.min.x + inset_x, building.height, f.min.y + inset_z],
                [f.max.x - inset_x, building.height + building.setback_height, f.max.y - inset_z],
                palette::ROOF,
                wall,
            );
        }
    }

    // street furniture
    for p in city.props() {
        match p.kind {
            PropKind::Tree => {
                let h = 2.6 * p.scale;
                m.box_shaded(
                    [p.pos.x - 0.13, 0.0, p.pos.y - 0.13],
                    [p.pos.x + 0.13, h, p.pos.y + 0.13],
                    palette::TRUNK,
                    palette::TRUNK,
                );
                let cr = 1.3 * p.scale;
                m.box_shaded(
                    [p.pos.x - cr, h, p.pos.y - cr],
                    [p.pos.x + cr, h + 1.9 * p.scale, p.pos.y + cr],
                    palette::LEAF,
                    [palette::LEAF[0] * 0.8, palette::LEAF[1] * 0.85, palette::LEAF[2] * 0.8],
                );
            }
            PropKind::Lamp => {
                let h = 5.4 * p.scale;
                m.box_shaded(
                    [p.pos.x - 0.10, 0.0, p.pos.y - 0.10],
                    [p.pos.x + 0.10, h, p.pos.y + 0.10],
                    palette::METAL,
                    palette::METAL,
                );
                let head = if p.glow > 0.05 {
                    palette::LAMP_ON
                } else {
                    palette::METAL
                };
                m.box_shaded(
                    [p.pos.x - 0.34, h, p.pos.y - 0.17],
                    [p.pos.x + 0.34, h + 0.24, p.pos.y + 0.16],
                    head,
                    head,
                );
            }
            PropKind::Monument => {
                let h = 6.0 * p.scale;
                m.box_shaded(
                    [p.pos.x - 0.9, 0.0, p.pos.y - 0.9],
                    [p.pos.x + 0.8, h, p.pos.y + 0.9],
                    palette::MONUMENT,
                    [0.52, 0.46, 0.58],
                );
            }
            PropKind::BusStop => {
                m.box_shaded(
                    [p.pos.x - 1.3, 0.0, p.pos.y - 0.5],
                    [p.pos.x + 1.0, 2.4, p.pos.y + 0.5],
                    [0.52, 0.48, 0.52],
                    [0.44, 0.42, 0.46],
                );
            }
            PropKind::Barrier | PropKind::Planter | PropKind::Bollard => {
                let w = if p.kind == PropKind::Bollard { 0.16 } else { 0.6 };
                let h = if p.kind == PropKind::Bollard { 0.85 } else { 1.1 };
                m.box_shaded(
                    [p.pos.x - w, 0.0, p.pos.y - w],
                    [p.pos.x + w, h, p.pos.y + w],
                    palette::CONCRETE,
                    palette::CONCRETE,
                );
            }
            _ => {}
        }
    }
}
