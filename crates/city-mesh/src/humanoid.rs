//! The humanoid **part palette**: an animated figure made of 11 boxes.
//!
//! No mesh file, no skeleton asset: the body is a list of [`Bone`]s, each a box defined in
//! its own *part-local* frame, with the joint at the frame origin. A [`PartPose`] rotates
//! every limb about its joint; composed down the kinematic chain and multiplied by a body
//! transform this gives one [`Mat4`] per bone — the palette the renderer uploads once per
//! frame and reuses for every agent, so the whole crowd animates from a single buffer.
//!
//! Conventions (pinned by `tests/mesh_rig.rs`):
//! * metres, right-handed, `Y` up; the figure stands on `y = 0` and grows up along `+Y`;
//! * a bone box **hangs from its joint**: in its own frame the box spans `y = -height..=0`,
//!   so thighs hang under hips and forearms under elbows while the pelvis box rides on top
//!   of the hip line;
//! * the pelvis origin **is the hip line**: `figure_frames` lifts a figure by
//!   [`HIP_HEIGHT`] (one straight leg), which is what puts the soles on the ground plane;
//! * the figure faces local **+Z**. The pelvis origin sits on the centre line, so a limb —
//!   which hinges about the body's X axis — swings in the `Y/Z` plane, straight fore and
//!   aft; `…L…` bones hang on local `-Z`, `…R…` bones on `+Z`;
//! * a **positive** limb angle swings the limb **forward**, toward +Z (a hanging bone tips
//!   that way under `city-math`'s `rotate_x`);
//! * a **positive** `torso_twist` turns the chest toward the figure's **left**, and a
//!   **positive** `head_pitch` nods the head forward — a nod **down**;
//! * a knee is never negative: a knee only ever bends backwards.

use city_math::{Mat4, Vec3};

use crate::builder::MeshBuilder;

/// Number of bones in the part palette.
pub const PART_COUNT: usize = 11;

/// A named bone. The discriminant is the palette slot index; [`PART_ORDER`] is the
/// canonical draw order (proximal → distal, torso before limbs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bone {
    Pelvis = 0,
    Torso = 1,
    Head = 2,
    ArmLUpper = 3,
    ArmLFore = 4,
    ArmRUpper = 5,
    ArmRFore = 6,
    LegLUpper = 7,
    LegLLower = 8,
    LegRUpper = 9,
    LegRLower = 10,
}

/// Canonical palette order (index == palette slot).
pub const PART_ORDER: [Bone; PART_COUNT] = [
    Bone::Pelvis,
    Bone::Torso,
    Bone::Head,
    Bone::ArmLUpper,
    Bone::ArmLFore,
    Bone::ArmRUpper,
    Bone::ArmRFore,
    Bone::LegLUpper,
    Bone::LegLLower,
    Bone::LegRUpper,
    Bone::LegRLower,
];

impl Bone {
    /// Palette slot of this bone.
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Bone at palette slot `i` (`None` outside the palette).
    pub fn from_index(i: usize) -> Option<Bone> {
        PART_ORDER.get(i).copied()
    }

    /// Parent bone (`None` for the pelvis, which carries the root transform).
    pub fn parent(self) -> Option<Bone> {
        match self {
            Bone::Pelvis => None,
            Bone::Torso | Bone::ArmLUpper | Bone::ArmRUpper | Bone::LegLUpper | Bone::LegRUpper => {
                Some(Bone::Pelvis)
            }
            Bone::Head => Some(Bone::Torso),
            Bone::ArmLFore => Some(Bone::ArmLUpper),
            Bone::ArmRFore => Some(Bone::ArmRUpper),
            Bone::LegLLower => Some(Bone::LegLUpper),
            Bone::LegRLower => Some(Bone::LegRUpper),
        }
    }

    /// `true` for the bones a pose rotates (the pelvis rides the body transform).
    #[inline]
    pub fn is_rotated(self) -> bool {
        !matches!(self, Bone::Pelvis)
    }

    /// `true` for the left-side limbs (the local `-Z` side of the body).
    #[inline]
    pub fn is_left(self) -> bool {
        matches!(
            self,
            Bone::ArmLUpper | Bone::ArmLFore | Bone::LegLUpper | Bone::LegLLower
        )
    }
}

/// A part's box in its own local frame: half-extents in X/Z, length along `+Y`, offset
/// from the parent's joint frame.
#[derive(Clone, Copy, Debug)]
pub struct PartGeom {
    pub bone: Bone,
    /// Joint position of this bone **in its parent's frame**.
    pub offset: Vec3,
    /// Half depth across the bone (local X).
    pub hx: f32,
    /// Half thickness of the bone (local Z).
    pub hz: f32,
    /// Length of the bone (metres).
    pub height: f32,
}

/// The canonical proportions of the palette (a 1.84 m figure).
///
/// Limbs are two boxes each (upper + lower), the torso a slab, the head a cube on a
/// short neck: enough silhouette to read as a person at 20 m, three triangles per face.
pub const PART_GEOM: [PartGeom; PART_COUNT] = [
    // Root box the figure hangs from, hanging *below* the hip line so the legs hinge on
    // the hip line and reach from there all the way down to the pavement.
    PartGeom {
        bone: Bone::Pelvis,
        offset: Vec3::ZERO,
        hx: 0.17,
        hz: 0.115,
        height: 0.16,
    },
    // Chest: stands on top of the pelvis box, above the hip line it hangs on.
    PartGeom {
        bone: Bone::Torso,
        offset: Vec3::new(0.0, 0.32, 0.0),
        hx: 0.235,
        hz: 0.135,
        height: 0.50,
    },
    // Head: the neck joint sits 8 cm above the top of the chest box, and the head box
    // hangs from it into the shoulders.
    PartGeom {
        bone: Bone::Head,
        offset: Vec3::new(0.0, 0.74, 0.0),
        hx: 0.115,
        hz: 0.125,
        height: 0.24,
    },
    // Shoulders hinge inside the top of the chest and hang down the sides; the figure's
    // left hangs on local -Z, its right on +Z.
    PartGeom {
        bone: Bone::ArmLUpper,
        offset: Vec3::new(0.0, 0.44, -0.27),
        hx: 0.060,
        hz: 0.065,
        height: 0.30,
    },
    PartGeom {
        bone: Bone::ArmLFore,
        // the elbow hinges one upper-arm below the shoulder; the forearm hangs from there
        offset: Vec3::new(0.0, -0.30, 0.0),
        hx: 0.055,
        hz: 0.058,
        height: 0.28,
    },
    PartGeom {
        bone: Bone::ArmRUpper,
        offset: Vec3::new(0.0, 0.44, 0.27),
        hx: 0.060,
        hz: 0.065,
        height: 0.30,
    },
    PartGeom {
        bone: Bone::ArmRFore,
        offset: Vec3::new(0.0, -0.30, 0.0),
        hx: 0.055,
        hz: 0.058,
        height: 0.28,
    },
    // Legs hinge on the hip line (the pelvis origin) and hang to the pavement: the knee
    // hinges one thigh below the hip and the shin hangs from there.
    PartGeom {
        bone: Bone::LegLUpper,
        offset: Vec3::new(0.0, 0.0, -0.105),
        hx: 0.075,
        hz: 0.080,
        height: 0.46,
    },
    PartGeom {
        bone: Bone::LegLLower,
        offset: Vec3::new(0.0, -0.46, 0.0),
        hx: 0.062,
        hz: 0.070,
        height: 0.46,
    },
    PartGeom {
        bone: Bone::LegRUpper,
        offset: Vec3::new(0.0, 0.0, 0.105),
        hx: 0.062,
        hz: 0.070,
        height: 0.46,
    },
    PartGeom {
        bone: Bone::LegRLower,
        offset: Vec3::new(0.0, -0.46, 0.0),
        hx: 0.062,
        hz: 0.070,
        height: 0.46,
    },
]; // Look up a part's geometry by bone.
#[inline]
pub fn part_geom(bone: Bone) -> &'static PartGeom {
    &PART_GEOM[bone.index()]
}

/// Palette slot of the pelvis (also usable in `const` context).
pub const SLOT_PELVIS: usize = 0;
pub const SLOT_TORSO: usize = 1;
pub const SLOT_HEAD: usize = 2;
pub const SLOT_ARM_L_UPPER: usize = 3;
pub const SLOT_ARM_L_FORE: usize = 4;
pub const SLOT_LEG_L_UPPER: usize = 7;
pub const SLOT_LEG_L_LOWER: usize = 8;

/// Thigh length (bone length, metres).
pub const THIGH: f32 = PART_GEOM[SLOT_LEG_L_UPPER].height;
/// Lower-leg length (bone), metres.
pub const SHIN: f32 = PART_GEOM[SLOT_LEG_L_LOWER].height;
/// Straight-leg length: thigh + shin measured along the bones, i.e. how far a straight
/// leg hangs down from the hip joint it swings on.
pub const LEG_LENGTH: f32 = THIGH + SHIN;

/// Upper-arm length (bone), metres.
pub const UPPER_ARM: f32 = PART_GEOM[SLOT_ARM_L_UPPER].height;
/// Forearm length (bone), metres.
pub const FOREARM: f32 = PART_GEOM[SLOT_ARM_L_FORE].height;

/// How far above the ground the pelvis origin of a standing figure rides: one leg, since
/// the legs hang from the hip line down to the sole.
pub const HIP_HEIGHT: f32 = LEG_LENGTH;

/// Head top above the pelvis origin: the neck joint is the crown, the head box hangs
/// below it into the shoulders.
const fn head_above_pelvis() -> f32 {
    PART_GEOM[SLOT_HEAD].offset.y
}

/// Total height of a standing figure, sole to crown: the hip ride plus the spine stack up
/// to the neck joint, plus the head box hanging from it (1.98 m for the stock bones).
pub const FIGURE_HEIGHT: f32 = HIP_HEIGHT + head_above_pelvis();

/// Height of the head crown above the ground for a figure standing on `y = 0`: the hip
/// line, the spine stack and the neck gap, i.e. the joint the head box hangs from.
pub const CROWN_HEIGHT: f32 =
    HIP_HEIGHT + PART_GEOM[SLOT_TORSO].offset.y + PART_GEOM[SLOT_TORSO].height + 0.08;

/// Vertices written per figure: 11 boxes × 6 quads × 6 verts.
pub const VERTS_PER_FIGURE: usize = PART_COUNT * 36;

/// How much of the chest twist the head counter-rotates.
///
/// A pure kinematic chain would swing the head 10 degrees off axis with every chest
/// twist — nobody walks around looking at the kerb. The head therefore takes the chest
/// yaw back out (`HEAD_COUNTER_TWIST`) and keeps only its own pitch: the head is still
/// *carried* by the chest (it moves with the body), it just does not stare at the kerb.
pub const HEAD_COUNTER_TWIST: f32 = 1.0;

/// A pose: one angle per rotated bone (radians) plus a vertical bob.
///
/// Angles are absolute *within* their parent frame (an elbow angle is measured from the
/// upper arm, `0` = straight). [`PartPose::from_avatar`] translates the vocabulary of
/// [`city_avatar::AvatarPose`]; the tests pin the direction of every axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartPose {
    /// Pelvis vertical offset (breathing / bob), metres.
    pub bob: f32,
    /// Chest twist about `Y` (`+` = toward the figure's left).
    pub torso_twist: f32,
    /// Chest lean (`+` = leaning forward).
    pub torso_pitch: f32,
    /// Head pitch (`+` = looking down).
    pub head_pitch: f32,
    /// Left upper arm swing (`+` = forward).
    pub arm_l: f32,
    /// Left elbow flex (never negative in practice).
    pub elbow_l: f32,
    pub arm_r: f32,
    pub elbow_r: f32,
    /// Left thigh (`+` = forward).
    pub leg_l: f32,
    /// Left knee flex (`0` = straight; bends backwards only).
    pub knee_l: f32,
    pub leg_r: f32,
    pub knee_r: f32,
}

impl Default for PartPose {
    /// Relaxed standing pose: straight limbs, shallow natural elbow.
    fn default() -> PartPose {
        PartPose {
            bob: 0.0,
            torso_twist: 0.0,
            torso_pitch: 0.0,
            head_pitch: 0.0,
            arm_l: 0.0,
            elbow_l: NATURAL_ELBOW,
            arm_r: 0.0,
            elbow_r: NATURAL_ELBOW,
            leg_l: 0.0,
            knee_l: 0.0,
            leg_r: 0.0,
            knee_r: 0.0,
        }
    }
}

/// Elbow flex of a hanging arm.
pub const NATURAL_ELBOW: f32 = 0.10;

impl PartPose {
    /// Translate a [`city_avatar::AvatarPose`] into part angles.
    ///
    /// `city_avatar` already emits "positive limb = forward", so the limbs copy 1:1. Two
    /// vocabularies differ and are translated here:
    /// * `torso_twist` takes the **opposite** sign — `city_avatar` twists the chest
    ///   *against* the leading arm, the palette twists *with* the leading leg;
    /// * `head_pitch` flips: the avatar poses the head "up-positive", the palette is
    ///   "down-positive".
    ///
    /// Elbows flex with the arm swing, knees with the backward sweep of the thigh
    /// ([`knee_flex`]), so a pose never asks a knee to bend forwards.
    pub fn from_avatar(p: &city_avatar::AvatarPose) -> PartPose {
        let flex = |a: f32| NATURAL_ELBOW + 0.55 * a.abs().min(1.0);
        PartPose {
            bob: p.bob,
            torso_twist: -p.torso_twist,
            torso_pitch: p.torso_pitch,
            head_pitch: -p.head_pitch,
            arm_l: p.arm_l,
            elbow_l: flex(p.arm_l),
            arm_r: p.arm_r,
            elbow_r: flex(p.arm_r),
            leg_l: p.leg_l,
            knee_l: knee_flex(p.leg_l),
            leg_r: p.leg_r,
            knee_r: knee_flex(p.leg_r),
        }
    }

    /// `true` when only the natural elbow flex is applied (the bind pose).
    pub fn is_neutral(&self) -> bool {
        self.bob.abs() < 1e-6
            && self.torso_twist.abs() < 1e-6
            && self.torso_pitch.abs() < 1e-6
            && self.head_pitch.abs() < 1e-6
            && self.arm_l.abs() < 1e-6
            && self.arm_r.abs() < 1e-6
            && self.leg_l.abs() < 1e-6
            && self.leg_r.abs() < 1e-6
            && self.knee_l.abs() < 1e-6
            && self.knee_r.abs() < 1e-6
            && (self.elbow_l - NATURAL_ELBOW).abs() < 1e-6
            && (self.elbow_r - NATURAL_ELBOW).abs() < 1e-6
    }

    /// A mid-stride walk pose at stride phase `phase01` (`0..1`) with amplitude `amp`.
    pub fn walk(phase01: f32, amp: f32) -> PartPose {
        let w = (phase01 - phase01.floor()) * city_math::TAU;
        let s = w.sin();
        // At phase 0.25 (s = +1) the left leg strides forward and the right one trails;
        // the arms answer contralaterally, and a back-sweeping leg folds its knee.
        PartPose {
            // the body sinks as each leg takes the weight and is level at the two
            // double-support moments: one dip per stride, never a lift
            bob: -0.035 * amp * w.sin().abs(),
            // the chest turns toward the striding leg
            torso_twist: 0.10 * amp * s,
            torso_pitch: 0.10 * amp,
            head_pitch: 0.0,
            arm_l: -0.85 * amp * s,
            elbow_l: NATURAL_ELBOW + 0.65 * amp * s.max(0.0),
            arm_r: 0.85 * amp * s,
            elbow_r: NATURAL_ELBOW + 0.65 * amp * (-s).max(0.0),
            leg_l: 0.55 * amp * s,
            knee_l: knee_flex(0.55 * amp * s),
            leg_r: -0.55 * amp * s,
            knee_r: knee_flex(-0.55 * amp * s),
        }
    }
}

/// A knee only bends backwards: a forward-swinging thigh keeps a nearly straight knee, a
/// back-sweeping one lifts the heel.
#[inline]
pub fn knee_flex(leg: f32) -> f32 {
    0.05 + 0.85 * (-leg).max(0.0)
}

/// The frame a pose rotates about: the **body** axes, not the bone's own.
///
/// A limb rotation is defined in the *body* frame — "swing forward" always means toward
/// the body's front, whichever way the parent segment happens to point — so the offset to
/// the joint is applied after the rotation. That keeps limbs attached to the body axis
/// (an elbow hinges on the elbow while the chest twists) and is what makes every pose
/// axis testable in isolation.
#[inline]
fn rotated(bone: Bone, rot: Mat4) -> Mat4 {
    let g = part_geom(bone);
    rot.mul(&Mat4::translation(g.offset))
}

/// Local transform of one bone relative to its parent at pose `pose`
/// (identity rotation for the pelvis).
pub fn part_local(bone: Bone, pose: &PartPose) -> Mat4 {
    let g = part_geom(bone);
    match bone {
        Bone::Pelvis => Mat4::IDENTITY,
        // Chest: twist about the spine, then lean. `rotate_x(+a)` tips a hanging bone
        // toward the figure's front (+Z); the chest *stands* on the pelvis and hangs the
        // other way, so the forward lean is the negated pitch. `rotate_y` turns the chest
        // toward the figure's left.
        Bone::Torso => Mat4::rotate_y(pose.torso_twist)
            .mul(&Mat4::rotate_x(-pose.torso_pitch))
            .mul(&Mat4::translation(g.offset)),
        // The head hangs from the neck and pitches the same way a limb swings: a positive
        // angle carries the face forward, i.e. a nod down. The chest twist is taken back
        // out (`HEAD_COUNTER_TWIST`): the shoulders carry the head along, they do not sling
        // it round to stare at the kerb.
        Bone::Head => Mat4::translation(g.offset)
            .mul(&Mat4::rotate_y(-pose.torso_twist * HEAD_COUNTER_TWIST))
            .mul(&Mat4::rotate_x(pose.head_pitch)),
        // Limbs hinge on the BODY axes: rotate first, then reach out to the joint, so a
        // forward swing stays forward whatever the parent segment does. A limb hangs along
        // -Y and `rotate_x(+a)` tips it toward +Z, the figure's front, so a positive limb
        // angle is a forward swing — the convention `PartPose` documents.
        Bone::ArmLUpper => rotated(bone, Mat4::rotate_x(pose.arm_l)),
        Bone::ArmRUpper => rotated(bone, Mat4::rotate_x(pose.arm_r)),
        Bone::ArmLFore => rotated(bone, Mat4::rotate_x(pose.elbow_l)),
        Bone::ArmRFore => rotated(bone, Mat4::rotate_x(pose.elbow_r)),
        Bone::LegLUpper => rotated(bone, Mat4::rotate_x(pose.leg_l)),
        Bone::LegRUpper => rotated(bone, Mat4::rotate_x(pose.leg_r)),
        Bone::LegLLower => rotated(bone, Mat4::rotate_x(pose.knee_l)),
        Bone::LegRLower => rotated(bone, Mat4::rotate_x(pose.knee_r)),
    }
}

/// Absolute frame of every bone: `body` (the feet transform) times the chain of part
/// locals. Output index == [`Bone::index`].
///
/// `PART_ORDER` lists parents before children, so a single pass resolves the chain.
pub fn part_frames(body: &Mat4, pose: &PartPose) -> [Mat4; PART_COUNT] {
    let mut out = [Mat4::IDENTITY; PART_COUNT];
    for bone in PART_ORDER.iter().copied() {
        let local = part_local(bone, pose);
        out[bone.index()] = match bone.parent() {
            None => body.mul(&local),
            Some(p) => out[p.index()].mul(&local),
        };
    }
    out
}

/// Palette-scaled matrices of every bone: `frames[i] * part_palette(bone)`, i.e. the
/// matrices that map the unit cube straight onto each bone box.
pub fn figure_mats(frames: &[Mat4; PART_COUNT]) -> [Mat4; PART_COUNT] {
    let mut out = [Mat4::IDENTITY; PART_COUNT];
    for bone in PART_ORDER.iter().copied() {
        let i = bone.index();
        out[i] = frames[i].mul(&part_palette(bone));
    }
    out
}

/// The **bind pose** frames: zero pose at the origin.
pub fn part_bind_frames() -> [Mat4; PART_COUNT] {
    part_frames(&Mat4::IDENTITY, &PartPose::default())
}

/// The palette matrix of one bone: maps the unit cube (`[-1,1]` cubed) onto the bone box
/// **in its own frame** (that box spans `y = 0..=height`, hence the half-height shift).
///
/// The renderer composes `part_frames(body, pose)[i] * part_palette(bone)` to get the
/// final bone matrix: frames carry the pose, the palette the proportions, so the palette
/// is constant for every figure and only the 11 frame matrices change per agent.
pub fn part_palette(bone: Bone) -> Mat4 {
    let g = part_geom(bone);
    let h = 0.5 * g.height;
    // Scale the unit cube by the bone's half-extents and hang it below its origin: in a
    // bone's own frame the box spans y = -height..=0, so arms hang at the sides, legs
    // under the hips, and the pelvis box rides on top of the hip line.
    Mat4::from_cols([
        [g.hx, 0.0, 0.0, 0.0],
        [0.0, h, 0.0, 0.0],
        [0.0, 0.0, g.hz, 0.0],
        [0.0, -h, 0.0, 1.0],
    ])
}

/// Body transform for a figure standing at `center` (feet, XZ) on height `ground`,
/// facing `yaw`, uniformly scaled about its own feet.
///
/// The pelvis of the palette sits at local `y = 0`, so the lift by [`HIP_HEIGHT`] happens
/// in [`figure_frames`], not here: `body` maps the palette origin to the feet.
pub fn body_matrix(center: [f32; 2], yaw: f32, ground: f32, scale: f32) -> Mat4 {
    Mat4::compose(
        Vec3::new(center[0], ground, center[1]),
        yaw,
        0.0,
        Vec3::new(scale, scale, scale),
    )
}

/// Frames of a whole figure, feet on `ground`: [`part_frames`] applied to a body that
/// lifts the pelvis by [`HIP_HEIGHT`] and applies the pose bob.
pub fn figure_frames(
    center: [f32; 2],
    yaw: f32,
    ground: f32,
    scale: f32,
    pose: &PartPose,
) -> [Mat4; PART_COUNT] {
    // The pelvis rides HIP_HEIGHT above the feet, and rides the pose bob; scaling is
    // applied about the palette origin so a smaller figure keeps its proportions.
    let body = Mat4::translation(Vec3::new(0.0, HIP_HEIGHT + pose.bob, 0.0))
        .mul(&body_matrix(center, yaw, ground, scale));
    part_frames(&body, pose)
}

/// The joint (origin) of a bone in world space.
#[inline]
pub fn joint_origin(frames: &[Mat4; PART_COUNT], bone: Bone) -> Vec3 {
    let m = &frames[bone.index()];
    Vec3::new(m.cols[3][0], m.cols[3][1], m.cols[3][2])
}

/// Palette of bone colours: shirt on the chest and upper arms, trousers on the pelvis
/// and legs, skin on head and forearms.
pub fn figure_colors(
    shirt: [f32; 3],
    trousers: [f32; 3],
    skin: [f32; 3],
) -> [[f32; 3]; PART_COUNT] {
    let mut c = [[0.0f32; 3]; PART_COUNT];
    for bone in PART_ORDER.iter().copied() {
        c[bone.index()] = match bone {
            Bone::Pelvis
            | Bone::LegLUpper
            | Bone::LegLLower
            | Bone::LegRUpper
            | Bone::LegRLower => trousers,
            Bone::Torso | Bone::ArmLUpper | Bone::ArmRUpper => shirt,
            Bone::Head | Bone::ArmLFore | Bone::ArmRFore => skin,
        };
    }
    c
}

/// Append one posed figure to `m`: 11 boxes, 36 vertices each.
///
/// `frames` come from [`figure_frames`] or [`part_frames`]; `colors[i]` is the colour of
/// [`PART_ORDER`]`[i]`.
pub fn append_figure(
    m: &mut MeshBuilder,
    frames: &[Mat4; PART_COUNT],
    colors: &[[f32; 3]; PART_COUNT],
) {
    for bone in PART_ORDER.iter().copied() {
        let i = bone.index();
        let mat = frames[i].mul(&part_palette(bone));
        append_unit_box(m, &mat, colors[i]);
    }
}

/// Tessellate the unit cube through `mat` (one face per quad, 6 quads / 36 vertices).
pub fn append_unit_box(m: &mut MeshBuilder, mat: &Mat4, col: [f32; 3]) {
    // unit-cube faces: outward normal + 4 corners, CCW seen from the normal
    const FACES: [([f32; 3], [[f32; 3]; 4]); 6] = [
        (
            [0.0, 1.0, 0.0],
            [
                [-1.0, 1.0, -1.0],
                [-1.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, -1.0],
            ],
        ),
        (
            [0.0, -1.0, 0.0],
            [
                [-1.0, -1.0, 1.0],
                [-1.0, -1.0, -1.0],
                [1.0, -1.0, -1.0],
                [1.0, -1.0, 1.0],
            ],
        ),
        (
            [0.0, 0.0, 1.0],
            [
                [-1.0, -1.0, 1.0],
                [1.0, -1.0, 1.0],
                [1.0, 1.0, 1.0],
                [-1.0, 1.0, 1.0],
            ],
        ),
        (
            [0.0, 0.0, -1.0],
            [
                [1.0, -1.0, -1.0],
                [-1.0, -1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [1.0, 1.0, -1.0],
            ],
        ),
        (
            [1.0, 0.0, 0.0],
            [
                [1.0, -1.0, 1.0],
                [1.0, -1.0, -1.0],
                [1.0, 1.0, -1.0],
                [1.0, 1.0, 1.0],
            ],
        ),
        (
            [-1.0, 0.0, 0.0],
            [
                [-1.0, -1.0, -1.0],
                [-1.0, -1.0, 1.0],
                [-1.0, 1.0, 1.0],
                [-1.0, 1.0, -1.0],
            ],
        ),
    ];
    for (n, corners) in FACES.iter() {
        let nn = mat.dir(Vec3::new(n[0], n[1], n[2])).norm();
        let p = |c: &[f32; 3]| mat.point(Vec3::new(c[0], c[1], c[2])).as_array();
        m.quad(
            p(&corners[0]),
            p(&corners[1]),
            p(&corners[2]),
            p(&corners[3]),
            nn.as_array(),
            col,
        );
    }
}
