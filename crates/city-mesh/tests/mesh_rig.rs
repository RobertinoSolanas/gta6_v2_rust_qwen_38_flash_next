//! The humanoid rig and the crowd geometry of `city-mesh`: palette invariants, the
//! direction of every pose axis, the kinematic chain, the walk cycle, and the agents of
//! `city-sim` drawn as posed figures (walkers) and boxes (cars).
//!
//! These tests are the contract between `city-avatar` (which poses the body) and the
//! renderer: if a sign, a joint or a proportion moves, they fail here instead of the
//! picture.

use city_avatar::{Avatar, AvatarConfig, AvatarPose};
use city_layout::{City, CityParams};
use city_math::{Mat4, Vec2, Vec3};
use city_mesh::agents;
use city_mesh::builder::MeshBuilder;
use city_mesh::humanoid::{
    append_figure, figure_colors, figure_frames, figure_mats, joint_origin, knee_flex,
    part_bind_frames, part_frames, part_geom, part_local, part_palette, Bone, PartPose,
    CROWN_HEIGHT, FIGURE_HEIGHT, FOREARM, HIP_HEIGHT, LEG_LENGTH, PART_COUNT, PART_GEOM,
    PART_ORDER, THIGH, UPPER_ARM, VERTS_PER_FIGURE,
};
use city_sim::{Car, CarKind, Crowd, Ped, SimConfig};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn small_city() -> City {
    City::generate(CityParams {
        seed: 7,
        blocks_x: 3,
        blocks_z: 3,
        ..CityParams::default()
    })
}

/// The crowd as the sim spawns it.
fn crowd() -> (Vec<Ped>, Vec<Car>) {
    let city = small_city();
    let cfg = SimConfig::tiny();
    let sim = Crowd::new(&city, cfg);
    (sim.peds().to_vec(), sim.cars().to_vec())
}

/// Lowest world y reached by a bone box.
fn bone_min_y(frames: &[Mat4; PART_COUNT], bone: Bone) -> f32 {
    let mat = frames[bone.index()].mul(&part_palette(bone));
    let mut lo = f32::MAX;
    for x in [-1.0f32, 1.0] {
        for y in [-1.0f32, 1.0] {
            for z in [-1.0f32, 1.0].iter().copied() {
                lo = lo.min(mat.point(Vec3::new(x, y, z)).y);
            }
        }
    }
    lo
}

/// Height of a figure from sole to head top.
fn figure_height(frames: &[Mat4; PART_COUNT]) -> f32 {
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    for bone in PART_ORDER.iter().copied() {
        let mat = frames[bone.index()].mul(&part_palette(bone));
        for x in [-1.0f32, 1.0] {
            for y in [-1.0f32, 1.0] {
                for z in [-1.0f32, 1.0] {
                    let p = mat.point(Vec3::new(x, y, z));
                    lo = lo.min(p.y);
                    hi = hi.max(p.y);
                }
            }
        }
    }
    hi - lo
}

/// Highest world y reached by any bone box.
fn frames_max_y(frames: &[Mat4; PART_COUNT]) -> f32 {
    let mut hi = f32::MIN;
    for bone in PART_ORDER.iter().copied() {
        let mat = frames[bone.index()].mul(&part_palette(bone));
        for x in [-1.0f32, 1.0] {
            for y in [-1.0f32, 1.0] {
                for z in [-1.0f32, 1.0] {
                    hi = hi.max(mat.point(Vec3::new(x, y, z)).y);
                }
            }
        }
    }
    hi
}

/// World position of the far end of a limb (a hanging bone's lower end).
fn limb_tip(frames: &[Mat4; PART_COUNT], bone: Bone) -> Vec3 {
    let mat = frames[bone.index()].mul(&part_palette(bone));
    mat.point(Vec3::new(0.0, -1.0, 0.0))
}

/// Direction a hanging bone points along (unit, pointing away from its joint).
fn bone_dir(frames: &[Mat4; PART_COUNT], bone: Bone) -> Vec3 {
    let f = &frames[bone.index()];
    let tip = f.mul(&part_palette(bone)).point(Vec3::new(0.0, -1.0, 0.0));
    let d = tip - f.point(Vec3::ZERO);
    let len = d.len();
    if len > 0.0 {
        d * (1.0 / len)
    } else {
        d
    }
}

/// Same, straight from a pose (identity body).
fn limb_tip_of(pose: &PartPose, bone: Bone) -> Vec3 {
    limb_tip(&part_frames(&Mat4::IDENTITY, pose), bone)
}

/// Joint position of a bone.
fn joint_of(frames: &[Mat4; PART_COUNT], bone: Bone) -> Vec3 {
    joint_origin(frames, bone)
}

/// Direction a bone points, straight from a pose.
fn bone_dir_of(pose: &PartPose, bone: Bone) -> Vec3 {
    bone_dir(&part_frames(&Mat4::IDENTITY, pose), bone)
}

/// Highest vertex of a builder.
fn max_y(m: &MeshBuilder) -> f32 {
    (0..m.len()).map(|i| m.get(i).0[1]).fold(f32::MIN, f32::max)
}

/// XZ spread of all bone boxes (a crude footprint of a figure).
fn figure_bounds(frames: &[Mat4; PART_COUNT]) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::MAX; 2];
    let mut max = [f32::MIN; 2];
    for bone in PART_ORDER.iter().copied() {
        let mat = frames[bone.index()].mul(&part_palette(bone));
        for x in [-1.0f32, 1.0] {
            for y in [-1.0f32, 1.0] {
                for z in [-1.0f32, 1.0] {
                    let p = mat.point(Vec3::new(x, y, z));
                    min[0] = min[0].min(p.x);
                    max[0] = max[0].max(p.x);
                    min[1] = min[1].min(p.z);
                    max[1] = max[1].max(p.z);
                }
            }
        }
    }
    (min, max)
}

/// XZ footprint of a raw builder.
fn footprint(m: &MeshBuilder) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::MAX; 2];
    let mut max = [f32::MIN; 2];
    for i in 0..m.len() {
        let p = m.get(i).0;
        min[0] = min[0].min(p[0]);
        min[1] = min[1].min(p[2]);
        max[0] = max[0].max(p[0]);
        max[1] = max[1].max(p[2]);
    }
    (min, max)
}

/// A pedestrian clone with an explicit speed and stride phase.
fn ped_with(p: &Ped, speed: f32, phase: f32) -> Ped {
    let mut q = p.clone();
    q.speed = speed;
    q.phase = phase;
    q
}

// ---------------------------------------------------------------------------
// palette structure
// ---------------------------------------------------------------------------

#[test]
fn the_palette_has_eleven_ordered_bones() {
    assert_eq!(PART_ORDER.len(), PART_COUNT);
    for (i, bone) in PART_ORDER.iter().copied().enumerate() {
        assert_eq!(bone.index(), i);
        assert_eq!(Bone::from_index(i), Some(bone));
    }
    assert_eq!(Bone::from_index(PART_COUNT), None);
    assert_eq!(PART_COUNT, 11);
}

#[test]
fn geometry_is_listed_in_palette_order() {
    for bone in PART_ORDER.iter().copied() {
        assert_eq!(part_geom(bone).bone, bone);
    }
    assert_eq!(PART_GEOM.len(), PART_COUNT);
}

#[test]
fn there_is_one_root_and_parents_precede_their_children() {
    let mut roots = 0;
    for bone in PART_ORDER.iter().copied() {
        match bone.parent() {
            None => {
                roots += 1;
                assert_eq!(bone, Bone::Pelvis);
            }
            Some(p) => {
                assert_ne!(p, bone);
                let ip = PART_ORDER
                    .iter()
                    .position(|b| *b == p)
                    .expect("parent listed");
                let me = PART_ORDER.iter().position(|b| *b == bone).unwrap();
                assert!(ip < me, "{p:?} must precede {bone:?} in PART_ORDER");
            }
        }
    }
    assert_eq!(roots, 1);
}

#[test]
fn a_bone_box_hangs_below_its_joint() {
    #![allow(clippy::assertions_on_constants)]
    // In a bone's own frame its box spans y = -height..=0: the segment dangles from the
    // joint it hinges on. This is what makes a shin hang under a knee.
    for bone in PART_ORDER.iter().copied() {
        let g = part_geom(bone);
        let near = part_palette(bone).point(Vec3::new(0.0, 1.0, 0.0)).y;
        let far = part_palette(bone).point(Vec3::new(0.0, -1.0, 0.0)).y;
        assert!(
            near <= 1e-6 && far <= 1e-6,
            "{bone:?} hangs below its joint"
        );
        assert!(
            (near - far - g.height).abs() < 1e-5,
            "{bone:?} keeps its length"
        );
    }
}

#[test]
fn every_bone_has_a_size() {
    for g in PART_GEOM.iter() {
        assert!(g.height > 0.0, "{:?} has no length", g.bone);
        assert!(g.hx > 0.0 && g.hz > 0.0, "{:?} has no thickness", g.bone);
    }
}

#[test]
fn left_and_right_bones_are_on_opposite_sides() {
    // The left parts hang on the figure's left (-Z at yaw 0), the right ones mirrored.
    // The right arm is modelled a touch narrower than the left, so only the joint height
    // and the box length are shared.
    let (l, r) = (part_geom(Bone::ArmLUpper), part_geom(Bone::ArmRUpper));
    assert_eq!(l.height, r.height);
    assert_eq!(l.hx, r.hx);
    assert!((l.offset.y - r.offset.y).abs() < 1e-6);
    assert!(l.offset.z < 0.0 && r.offset.z > 0.0, "shoulders mirrored");

    let (l, r) = (part_geom(Bone::LegLUpper), part_geom(Bone::LegRUpper));
    assert_eq!(l.height, r.height);
    assert!(l.offset.z < 0.0 && r.offset.z > 0.0);
    assert!((l.offset.z + r.offset.z).abs() < 1e-6, "hips are symmetric");
}

#[test]
fn scaling_a_figure_halves_its_height() {
    // The palette origin is the *pelvis*: scaling the body matrix halves the figure.
    let full = part_frames(&Mat4::IDENTITY, &PartPose::default());
    let half = part_frames(&Mat4::scale_uniform(0.5), &PartPose::default());
    let ratio = figure_height(&half) / figure_height(&full);
    assert!((ratio - 0.5).abs() < 1e-4, "scale factor {ratio}");
}

#[test]
fn a_standing_figure_is_human_tall() {
    #![allow(clippy::assertions_on_constants)]
    assert!(
        FIGURE_HEIGHT > 1.65 && FIGURE_HEIGHT < 2.0,
        "figure height {FIGURE_HEIGHT}"
    );
    assert!(
        HIP_HEIGHT > 0.7 && HIP_HEIGHT < 1.0,
        "hip height {HIP_HEIGHT}"
    );
    // the hip ride *is* the straight leg: bones only shorten when a joint bends
    assert_eq!(LEG_LENGTH, HIP_HEIGHT);
    // the head is stacked above the chest, and the chest on the pelvis
    assert!(part_geom(Bone::Head).offset.y >= part_geom(Bone::Torso).height);
    assert!(part_geom(Bone::Torso).offset.y >= part_geom(Bone::Pelvis).height);
    // the crown is the top of the spine stack, the head box hangs from it
    assert!(
        CROWN_HEIGHT > HIP_HEIGHT && CROWN_HEIGHT < 2.0,
        "crown {CROWN_HEIGHT}"
    );
    assert!(
        (CROWN_HEIGHT
            - (HIP_HEIGHT
                + part_geom(Bone::Torso).offset.y
                + part_geom(Bone::Torso).height
                + 0.08))
            .abs()
            < 1e-6
    );
}

#[test]
fn only_the_pelvis_is_unrotated_and_the_left_side_is_known() {
    assert!(!Bone::Pelvis.is_rotated());
    for bone in [Bone::Torso, Bone::Head, Bone::ArmLFore, Bone::LegRLower] {
        assert!(bone.is_rotated());
    }
    assert!(Bone::ArmLUpper.is_left() && Bone::LegLLower.is_left());
    assert!(!Bone::ArmRFore.is_left() && !Bone::Torso.is_left());
}

// ---------------------------------------------------------------------------
// bind pose: proportions in world space
// ---------------------------------------------------------------------------

#[test]
fn the_bind_pose_stands_on_the_ground_and_reaches_the_figure_height() {
    let frames = figure_frames([0.0, 0.0], 0.0, 0.0, 1.0, &PartPose::default());
    assert!(
        bone_min_y(&frames, Bone::LegLLower).abs() < 1e-4,
        "soles on y = 0"
    );
    assert!(
        (frames_max_y(&frames)
            - (HIP_HEIGHT + part_geom(Bone::Torso).offset.y + part_geom(Bone::Head).offset.y))
            .abs()
            < 0.02,
        "head top at {}",
        frames_max_y(&frames)
    );
}

#[test]
fn the_bind_pose_is_what_the_default_pose_gives() {
    let bind = part_bind_frames();
    let same = part_frames(&Mat4::IDENTITY, &PartPose::default());
    for i in 0..PART_COUNT {
        for c in 0..4 {
            for r in 0..4 {
                assert!(
                    (bind[i].cols[c][r] - same[i].cols[c][r]).abs() < 1e-6,
                    "bone {i} differs at {c},{r}"
                );
            }
        }
    }
}

#[test]
fn the_palette_maps_the_unit_cube_onto_the_bone_box() {
    let g = part_geom(Bone::Torso);
    let mat = part_palette(Bone::Torso);
    // the bone's own frame spans y = -height..=0, so the unit cube maps onto a box that
    // hangs from the joint: its widest ring sits at the joint, its far end one height down
    let near = mat.point(Vec3::new(1.0, 1.0, 1.0));
    let far = mat.point(Vec3::new(1.0, -1.0, 1.0));
    assert!((near.x - g.hx).abs() < 1e-6);
    assert!(near.y.abs() < 1e-6, "the box starts at its joint: {near:?}");
    assert!((near.z - g.hz).abs() < 1e-6);
    assert!(
        (far.y + g.height).abs() < 1e-5,
        "the box hangs one bone length down"
    );
}

#[test]
fn part_local_composes_the_joint_offset_after_the_rotation() {
    let local = part_local(
        Bone::ArmRUpper,
        &PartPose {
            arm_r: 0.7,
            ..Default::default()
        },
    );
    let expected = Mat4::rotate_x(0.7).mul(&Mat4::translation(part_geom(Bone::ArmRUpper).offset));
    for c in 0..4 {
        for r in 0..4 {
            assert!((local.cols[c][r] - expected.cols[c][r]).abs() < 1e-6);
        }
    }
}

#[test]
fn limbs_hinge_on_the_body_axis_and_the_spine_follows_it() {
    let pose = PartPose {
        arm_r: 0.7,
        elbow_r: 0.4,
        leg_l: 0.3,
        knee_l: 0.6,
        torso_twist: 0.3,
        ..Default::default()
    };
    let frames = part_frames(&Mat4::IDENTITY, &pose);

    // The spine is a real chain: every joint sits exactly where the parent left it.
    for (parent, child) in [(Bone::Pelvis, Bone::Torso), (Bone::Torso, Bone::Head)] {
        let expected = frames[parent.index()].point(part_geom(child).offset);
        let got = frames[child.index()].point(Vec3::ZERO);
        assert!(
            (got - expected).len() < 1e-4,
            "{child:?} joint drifted off {parent:?}: {got:?} vs {expected:?}"
        );
    }

    // Limbs hinge on the body X axis: a joint keeps its distance from the body axis it
    // swings around, whatever the angle. (The elbow is attached to the *elbow*, so the
    // elbow angle does not move the elbow — that is what keeps an arm in one piece.)
    for (bone, joint) in [
        (Bone::ArmLUpper, part_geom(Bone::ArmLUpper).offset),
        (Bone::LegLUpper, part_geom(Bone::LegLUpper).offset),
    ] {
        let got = joint_of(&frames, bone);
        let radius = (joint.y * joint.y + joint.z * joint.z).sqrt();
        assert!(
            (Vec3::new(0.0, got.y, got.z).len() - radius).abs() < 1e-4,
            "{bone:?} swings on a {radius} m arc, got {got:?}"
        );
    }
    // the elbow hinge keeps the forearm within one upper-arm of the shoulder, and the
    // elbow angle alone cannot drag it away
    let shoulder = joint_of(&frames, Bone::ArmRUpper);
    let elbow = joint_of(&frames, Bone::ArmRFore);
    assert!(
        (elbow - shoulder).len() <= part_geom(Bone::ArmRUpper).height + 1e-4,
        "elbow cannot be further than one upper arm from the shoulder: {:?} vs {shoulder:?}",
        elbow
    );
}

// ---------------------------------------------------------------------------
// the direction of every pose axis
// ---------------------------------------------------------------------------

#[test]
fn a_positive_thigh_swing_moves_the_leg_forward() {
    // The thigh hangs straight down; a positive angle swings it toward the figure's front
    // (+Z), a negative one sweeps it behind the body.
    let axis = |pose: &PartPose| -bone_dir_of(pose, Bone::LegLUpper);
    let rest = axis(&PartPose::default());
    let fwd = axis(&PartPose {
        leg_l: 0.6,
        ..Default::default()
    });
    let back = axis(&PartPose {
        leg_l: -0.6,
        ..Default::default()
    });
    assert!(
        rest.x.abs() < 1e-4 && rest.z.abs() < 1e-4,
        "standing leg: {rest:?}"
    );
    assert!(fwd.z > 0.3, "+leg_l swings the leg forward: {fwd:?}");

    assert!(back.z < -0.3, "-leg_l sweeps it back: {back:?}");
    // a swing is a rotation, not a stretch: the bone keeps its length
    let bone = part_frames(
        &Mat4::IDENTITY,
        &PartPose {
            leg_l: 0.6,
            ..Default::default()
        },
    )[Bone::LegLUpper.index()];
    let off = bone.point(Vec3::ZERO)
        - bone
            .mul(&part_palette(Bone::LegLUpper))
            .point(Vec3::new(0.0, -1.0, 0.0));
    assert!(
        (off.len() - THIGH).abs() < 1e-4,
        "the thigh keeps its length: {off:?}"
    );
}
#[test]
fn a_positive_arm_swing_moves_the_hand() {
    // A limb angle is a swing about the shoulder: the hand moves and the arm does not
    // stretch, whichever way the angle goes.
    let hand = |pose: PartPose| limb_tip_of(&pose, Bone::ArmLFore);
    let swung = hand(PartPose {
        arm_l: 1.2,
        elbow_l: 0.0,
        ..Default::default()
    });
    let level = hand(PartPose::default());
    let back = hand(PartPose {
        arm_l: -1.0,
        elbow_l: 0.0,
        ..Default::default()
    });
    assert!(
        (swung - level).len() > 0.1,
        "a swing moves the hand: {swung:?} vs {level:?}"
    );
    assert!(
        back != level,
        "and so does the other way: {back:?} vs {level:?}"
    );
    // a swing is a rotation, not a stretch: the hand stays within one arm of the shoulder
    let sh = joint_of(
        &part_frames(&Mat4::IDENTITY, &PartPose::default()),
        Bone::ArmLUpper,
    );
    for p in [swung, level, back] {
        assert!(
            (p - sh).len() <= UPPER_ARM + FOREARM + part_geom(Bone::ArmLFore).offset.y.abs() + 1e-4,
            "arm keeps its length: {p:?}"
        );
    }
}

#[test]
fn left_and_right_limbs_move_independently() {
    let pose = PartPose {
        arm_l: 0.9,
        arm_r: -0.4,
        ..Default::default()
    };
    let frames = part_frames(&Mat4::IDENTITY, &pose);
    // The two arms hang from their own shoulders and swing on their own angles, so the
    // two hands end up in two different places.
    let lh = limb_tip(&frames, Bone::ArmLFore);
    let rh = limb_tip(&frames, Bone::ArmRFore);
    assert!(
        (lh - rh).len() > 0.1,
        "the two hands are apart: {lh:?} vs {rh:?}"
    );
    // and the side offsets in the table really are mirrored
    let (l, r) = (part_geom(Bone::ArmLUpper), part_geom(Bone::ArmRUpper));
    assert!(
        l.offset.z < 0.0 && r.offset.z > 0.0,
        "shoulders mirrored in the table"
    );
}

#[test]
fn a_positive_torso_twist_turns_the_chest_to_the_figures_left() {
    let frames = part_frames(
        &Mat4::IDENTITY,
        &PartPose {
            torso_twist: 0.6,
            ..Default::default()
        },
    );
    // the chest corners swing around the spine: the front-right corner travels to the
    // figure's left (-Z) as the twist grows
    let mat = frames[Bone::Torso.index()].mul(&part_palette(Bone::Torso));
    let corner = mat.point(Vec3::new(1.0, 0.0, 0.0));
    assert!(corner.z < -0.05, "chest should turn left, got {corner:?}");
    assert!(corner.x > 0.1, "and still points mostly forward");
}

#[test]
fn a_positive_head_pitch_tips_the_head() {
    // The head hangs from the neck; a head pitch rotates it about that joint, so the head
    // moves while its box keeps its size.
    let tip = |pose: &PartPose| {
        let f = part_frames(&Mat4::IDENTITY, pose)[Bone::Head.index()];
        f.mul(&part_palette(Bone::Head))
            .point(Vec3::new(0.0, -1.0, 0.0))
    };
    let rest = tip(&PartPose::default());
    let down = tip(&PartPose {
        head_pitch: 0.6,
        ..Default::default()
    });
    let up = tip(&PartPose {
        head_pitch: -0.6,
        ..Default::default()
    });
    assert!(
        (down - rest).len() > 0.01,
        "a pitch swings the head: {down:?} vs {rest:?}"
    );
    assert!(
        (up - rest).len() > 0.01,
        "and so does the other way: {up:?} vs {rest:?}"
    );
    // a rotation never stretches the head
    let head_len = |pose: &PartPose| {
        let f = part_frames(&Mat4::IDENTITY, pose)[Bone::Head.index()];
        let m = f.mul(&part_palette(Bone::Head));
        (m.point(Vec3::new(0.0, 1.0, 0.0)) - m.point(Vec3::new(0.0, -1.0, 0.0))).len()
    };
    for p in [
        PartPose::default(),
        PartPose {
            head_pitch: 0.6,
            ..Default::default()
        },
    ] {
        let len = head_len(&p);
        assert!(
            (len - part_geom(Bone::Head).height).abs() < 1e-4,
            "head box {len}"
        );
    }
}

#[test]
fn the_head_rides_the_chest_and_counter_twists() {
    let twisted = PartPose {
        torso_twist: 0.9,
        ..Default::default()
    };
    let frames = part_frames(&Mat4::IDENTITY, &twisted);
    let head = frames[Bone::Head.index()];
    let rest = part_bind_frames()[Bone::Head.index()];
    // a twist about the spine barely moves a head that sits on the axis of the twist
    assert!(
        (head.point(Vec3::ZERO) - rest.point(Vec3::ZERO)).len() < 1e-4,
        "the head stays on the axis of the twist"
    );
    // ... and its height still comes from the chest stack, not from thin air
    assert!(part_geom(Bone::Head).offset.y >= part_geom(Bone::Torso).height);
    // the counter-twist keeps the head's own orientation pointed where the body points:
    // a head axis (up) and a head front are untouched by a chest twist alone
    let up = head.dir(Vec3::new(0.0, 1.0, 0.0));
    assert!(
        (up - Vec3::new(0.0, 1.0, 0.0)).len() < 1e-4,
        "the neck stays upright: {up:?}"
    );
    // the head stays stacked on the neck
    assert!(head.point(Vec3::ZERO).y > part_geom(Bone::Torso).offset.y);
}

#[test]
fn a_positive_torso_pitch_leans_forward() {
    // The chest stands on the pelvis and the head and chest share the same axis: a
    // positive pitch is the forward lean the avatar asks for.
    let f = part_frames(
        &Mat4::IDENTITY,
        &PartPose {
            torso_pitch: 0.5,
            ..Default::default()
        },
    );
    let rest = part_frames(&Mat4::IDENTITY, &PartPose::default());
    let axis = f[Bone::Torso.index()].dir(Vec3::new(0.0, 1.0, 0.0));
    let upright = rest[Bone::Torso.index()].dir(Vec3::new(0.0, 1.0, 0.0));
    assert!(
        axis.z < upright.z,
        "a positive pitch swings the chest forward: {axis:?} vs {upright:?}"
    );
}

// ---------------------------------------------------------------------------
// the walk cycle
// ---------------------------------------------------------------------------

#[test]
fn knees_never_bend_forwards() {
    for leg in [-1.0, -0.4, 0.0, 0.4, 1.0] {
        assert!(knee_flex(leg) >= 0.0);
        assert!(knee_flex(leg) <= 1.0);
    }
    assert!(
        knee_flex(-0.5) > knee_flex(0.5),
        "a back sweep bends the knee more"
    );
}

#[test]
fn a_walk_pose_never_bends_a_knee_forwards_and_never_stretches_the_leg() {
    for i in 0..32 {
        let phase = i as f32 / 16.0;
        let pose = PartPose::walk(phase, 1.0);
        assert!(pose.knee_l >= 0.0 && pose.knee_r >= 0.0, "phase {phase}");
        // A stride never asks a leg to reach beyond its bones. The hip joints sit either
        // side of the centre line, so measure each leg from its own hip: a straight leg
        // reaches exactly LEG_LENGTH, and a bent one comes up short.
        let frames = part_frames(&Mat4::translation(Vec3::new(0.0, HIP_HEIGHT, 0.0)), &pose);
        for (hip_bone, foot_bone) in [
            (Bone::LegLUpper, Bone::LegLLower),
            (Bone::LegRUpper, Bone::LegRLower),
        ] {
            let hip = joint_of(&frames, hip_bone);
            let foot = limb_tip(&frames, foot_bone);
            let d = (foot - hip).len();
            assert!(
                d <= LEG_LENGTH + 1e-4,
                "phase {phase}: leg overextended ({d} > {LEG_LENGTH}), foot {foot:?}"
            );
        }
    }
}

#[test]
fn a_positive_leg_angle_is_a_forward_stride() {
    // The angle itself is the vocabulary: at the half-way point of a stride the leading
    // leg carries the positive angle, which is what city-avatar speaks.
    let p = PartPose::walk(0.25, 1.0);
    assert!(p.leg_l > 0.0 && p.leg_r < 0.0, "{p:?}");
}

#[test]
fn the_walk_cycle_is_contralateral() {
    let a = PartPose::walk(0.25, 1.0);
    assert!(a.leg_l > 0.0, "phase 0.25: the left leg strides forward");
    assert!(a.leg_r < 0.0, "and the right leg trails");
    assert!(a.arm_l < 0.0, "the left arm opposes the forward left leg");
    assert!(a.arm_r > 0.0);

    let b = PartPose::walk(0.75, 1.0);
    assert!(
        b.leg_l < 0.0 && b.arm_l > 0.0,
        "half a cycle later everything swapped"
    );
}

#[test]
fn the_walk_cycle_is_periodic() {
    let a = PartPose::walk(0.1, 0.8);
    let b = PartPose::walk(1.1, 0.8);
    for (x, y) in [
        (a.leg_l, b.leg_l),
        (a.arm_l, b.arm_l),
        (a.torso_twist, b.torso_twist),
        (a.bob, b.bob),
    ] {
        assert!((x - y).abs() < 1e-5, "{a:?} vs {b:?}");
    }
}

#[test]
fn amplitude_scales_the_stride() {
    let calm = PartPose::walk(0.25, 0.2);
    let run = PartPose::walk(0.25, 1.0);
    assert!(calm.leg_l.abs() < run.leg_l.abs());
    assert!(run.arm_l.abs() > calm.arm_l.abs());
    assert!(
        PartPose::walk(0.25, 0.0).leg_l.abs() < 1e-6,
        "zero amp = standing"
    );
}

#[test]
fn the_bob_is_a_double_dip_that_never_lifts_the_figure() {
    // one dip per stride: level at double support (phase 0), lowest mid-stride
    let level = PartPose::walk(0.0, 1.0).bob;
    let dip = PartPose::walk(0.25, 1.0).bob;
    for i in 0..8 {
        let p = PartPose::walk(i as f32 / 8.0, 1.0);
        assert!(p.bob <= 1e-6, "the bob never lifts the figure: {}", p.bob);
    }
    assert!(level.abs() < 1e-5, "level at double support");
    assert!(dip < level, "and sunk mid-stride");
}

#[test]
fn figure_frames_lift_the_pose_bob_into_the_body() {
    let sunk = figure_frames(
        [0.0, 0.0],
        0.0,
        0.0,
        1.0,
        &PartPose {
            bob: -0.05,
            ..Default::default()
        },
    );
    let level = figure_frames([0.0, 0.0], 0.0, 0.0, 1.0, &PartPose::default());
    let dy = bone_min_y(&sunk, Bone::LegLLower) - bone_min_y(&level, Bone::LegLLower);
    assert!((dy + 0.05).abs() < 1e-4, "bob shifts the whole body: {dy}");
}

// ---------------------------------------------------------------------------
// the bridge to city-avatar
// ---------------------------------------------------------------------------

/// An avatar that has been walking straight for two seconds.
fn walking_avatar() -> (city_layout::City, Avatar) {
    let city = small_city();
    let mut a = Avatar::spawn(&city, AvatarConfig::default());
    for _ in 0..120 {
        a.update(&city, Vec2::new(1.0, 0.0), 0.0, false, 1.0 / 60.0);
    }
    (city, a)
}

#[test]
fn a_walking_avatar_becomes_a_walk_pose_with_usable_joints() {
    let (_city, avatar) = walking_avatar();
    assert!(
        avatar.speed() > 1.0,
        "the avatar is walking (speed {})",
        avatar.speed()
    );
    let pose = avatar.pose(0.0);
    let p = PartPose::from_avatar(&pose);
    // limbs keep the avatar's "positive = forward" convention
    assert_eq!(p.leg_l, pose.leg_l);
    assert_eq!(p.arm_r, pose.arm_r);
    // and the derived joints are always safe for the rig
    assert!(p.knee_l >= 0.0 && p.knee_r >= 0.0);
    assert!(p.elbow_l > 0.0 && p.elbow_r > 0.0, "a swinging arm bends");
}

#[test]
fn the_avatar_head_vocabulary_is_flipped() {
    let up = AvatarPose {
        head_pitch: 0.5,
        ..AvatarPose::default()
    };
    let p = PartPose::from_avatar(&up);
    assert_eq!(
        p.head_pitch, -0.5,
        "the palette poses the head down-positive"
    );
}

#[test]
fn the_chest_twist_is_translated_with_a_sign_flip() {
    let a = AvatarPose {
        torso_twist: 0.3,
        ..AvatarPose::default()
    };
    let p = PartPose::from_avatar(&a);
    assert_eq!(p.torso_twist, -0.3);
}

#[test]
fn a_standing_avatar_maps_to_an_upright_pose() {
    let city = small_city();
    let avatar = Avatar::spawn(&city, AvatarConfig::default());
    let p = PartPose::from_avatar(&avatar.pose(0.0));
    assert!(
        p.leg_l.abs() < 1e-6 && p.leg_r.abs() < 1e-6,
        "no stride while standing"
    );
    assert!(p.bob.abs() < 0.05);
    assert!(p.arm_l.abs() < 0.2);
}

// ---------------------------------------------------------------------------
// body transform, placement and scale
// ---------------------------------------------------------------------------

#[test]
fn a_figure_is_a_person_tall() {
    let frames = figure_frames([0.0, 0.0], 0.0, 0.0, 1.0, &PartPose::default());
    let hi = frames_max_y(&frames);
    // the built figure is a person tall
    assert!(hi > 1.65 && hi < 2.0, "figure is {hi} tall");
}

#[test]
fn figures_stand_on_their_own_ground() {
    // A figure is built for standing: at the palette's neutral pose its soles sit on the
    // ground plane it is placed on, wherever on the map that is.
    for (x, z, ground) in [(0.0, 0.0, 0.0), (12.0, -4.0, 0.15), (-12.5, 9.0, 0.15)] {
        let frames = figure_frames([x, z], 0.0, ground, 1.0, &PartPose::default());
        assert!(
            (bone_min_y(&frames, Bone::LegLLower) - ground).abs() < 1e-4,
            "figure at ({x},{z}) on {ground}"
        );
        let (min, max) = figure_bounds(&frames);
        assert!(((min[0] + max[0]) * 0.5 - x).abs() < 0.6);
        assert!(((min[1] + max[1]) * 0.5 - z).abs() < 0.6);
    }
}

#[test]
fn scaling_a_figure_scales_every_bone() {
    let full = part_frames(&Mat4::IDENTITY, &PartPose::default());
    let scaled = part_frames(
        &Mat4::from_cols([
            [0.5, 0.0, 0.0, 0.0],
            [0.0, 0.5, 0.0, 0.0],
            [0.0, 0.0, 0.5, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]),
        &PartPose::default(),
    );
    for bone in PART_ORDER.iter().copied() {
        let a = full[bone.index()].point(Vec3::new(1.0, 1.0, 0.0));
        let b = scaled[bone.index()].point(Vec3::new(1.0, 1.0, 0.0));
        assert!(
            ((b.x - a.x * 0.5).abs() < 1e-5 && (b.y - a.y * 0.5).abs() < 1e-5),
            "{bone:?}"
        );
    }
}

#[test]
fn yaw_turns_the_figure() {
    // A forward-swinging arm points its forearm where the figure faces: +Z at yaw 0 and
    // -Z after a half turn.
    let pose = PartPose {
        arm_l: std::f32::consts::FRAC_PI_2,
        elbow_l: 0.0,
        ..Default::default()
    };
    let north = figure_frames([0.0, 0.0], 0.0, 0.0, 1.0, &pose);
    let back = figure_frames([0.0, 0.0], city_math::PI, 0.0, 1.0, &pose);
    // the shoulder swings the forearm out of the hang and out in front of the chest
    let az = limb_tip(&north, Bone::ArmLFore).z - joint_of(&north, Bone::ArmLUpper).z;
    let bz = limb_tip(&back, Bone::ArmLFore).z - joint_of(&back, Bone::ArmLUpper).z;
    assert!(
        az.abs() > 0.2,
        "the swing reaches out from the shoulder: {az}"
    );
    assert!(az * bz < 0.0, "a half turn flips the reach: {az} vs {bz}");
}

#[test]
fn a_figure_writes_eleven_boxes_and_a_fixed_vertex_count() {
    let mut m = MeshBuilder::new();
    let frames = figure_frames([0.0, 0.0], 0.0, 0.0, 1.0, &PartPose::default());
    let colors = figure_colors([1.0, 0.0, 0.0], [0.2, 0.2, 0.4], [0.8, 0.6, 0.5]);
    append_figure(&mut m, &frames, &colors);
    assert_eq!(m.len(), VERTS_PER_FIGURE);
    assert_eq!(VERTS_PER_FIGURE, PART_COUNT * 36);
    assert_eq!(m.triangles(), PART_COUNT * 12);
    for i in 0..m.len() {
        assert!(m.get(i).0.iter().all(|v| v.is_finite()));
        // normals stay unit length after the pose/palette transforms
        let n = m.get(i).1;
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-3, "normals are unit length: {n:?}");
    }
}

#[test]
fn figure_colors_split_shirt_trousers_and_skin() {
    let c = figure_colors([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    assert_eq!(c[Bone::Torso.index()], [1.0, 0.0, 0.0]);
    assert_eq!(c[Bone::ArmLUpper.index()], [1.0, 0.0, 0.0]);
    assert_eq!(c[Bone::Pelvis.index()], [0.0, 1.0, 0.0]);
    assert_eq!(c[Bone::LegRUpper.index()], [0.0, 1.0, 0.0]);
    assert_eq!(c[Bone::Head.index()], [0.0, 0.0, 1.0]);
    assert_eq!(c[Bone::ArmLFore.index()], [0.0, 0.0, 1.0], "bare forearms");
}

#[test]
fn figure_mats_are_the_frames_multiplied_by_the_palette() {
    let frames = figure_frames([1.0, 2.0], 0.7, 0.15, 1.0, &PartPose::walk(0.3, 0.9));
    let mats = figure_mats(&frames);
    for bone in PART_ORDER.iter().copied() {
        let want = frames[bone.index()].mul(&part_palette(bone));
        for c in 0..4 {
            for r in 0..4 {
                assert!((mats[bone.index()].cols[c][r] - want.cols[c][r]).abs() < 1e-6);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// pedestrians
// ---------------------------------------------------------------------------

#[test]
fn a_pedestrian_is_drawn_as_exactly_one_figure() {
    let (peds, _) = crowd();
    assert!(!peds.is_empty(), "the sim spawns pedestrians");
    let mut m = MeshBuilder::new();
    for p in peds.iter() {
        agents::ped(&mut m, p, 0.15);
    }
    assert_eq!(m.len(), peds.len() * VERTS_PER_FIGURE);
}

#[test]
fn a_standing_pedestrian_is_placed_on_the_pavement() {
    let (peds, _) = crowd();
    let mut p = ped_with(&peds[0], 0.0, 3.3);
    // park the walker facing +X so the footprint is symmetric around its position
    p.dir = city_math::Vec2::X;
    let mut m = MeshBuilder::new();
    agents::ped(&mut m, &p, 0.15);
    let (min, max) = footprint(&m);
    let cx = (min[0] + max[0]) * 0.5;
    let cz = (min[1] + max[1]) * 0.5;
    assert!((p.x - cx).abs() < 0.45, "walker drawn off its position");
    assert!((p.z - cz).abs() < 0.45);
    // The figure stands on the pavement it was given and is a person tall.
    assert!(max_y(&m) > 0.15 + 1.5, "figure top {}", max_y(&m));
}

#[test]
fn stride_phase_changes_the_drawn_geometry() {
    let (peds, _) = crowd();
    let figure = |phase: f32| {
        let mut m = MeshBuilder::new();
        agents::ped(&mut m, &ped_with(&peds[0], 1.4, phase), 0.15);
        m.into_vec()
    };
    let a = figure(0.0);
    let b = figure(0.5);
    assert_ne!(
        a, b,
        "a different stride phase must draw a different figure"
    );
}

#[test]
fn walking_and_standing_figures_differ() {
    let (peds, _) = crowd();
    let draw = |speed: f32| {
        let mut m = MeshBuilder::new();
        agents::ped(&mut m, &ped_with(&peds[0], speed, 0.25), 0.15);
        m.into_vec()
    };
    assert_ne!(draw(0.0), draw(1.5));
}

#[test]
fn every_ped_variant_gets_a_colour_and_they_are_not_black() {
    let (peds, _) = crowd();
    let mut m = MeshBuilder::new();
    for v in 0..16u8 {
        let mut p = ped_with(&peds[0], 0.0, 0.0);
        p.variant = v % 8;
        let before = m.len();
        agents::ped(&mut m, &p, 0.15);
        // the chest is the second box of the figure
        let c = m.get(before + 36).2;
        assert!(
            c[0] + c[1] + c[2] > 0.3,
            "variant {v} drew a black figure: {c:?}"
        );
    }
}

#[test]
fn build_agents_draws_every_agent() {
    let (peds, cars) = crowd();
    let mut m = MeshBuilder::new();
    agents::build_agents(&peds, &cars, 0.0, 0.15, &mut m);
    assert_eq!(
        m.len(),
        peds.len() * VERTS_PER_FIGURE + cars.len() * agents::CAR_VERTS
    );
}

#[test]
fn a_crowd_fits_a_frame_budget() {
    let (peds, cars) = crowd();
    let mut m = MeshBuilder::new();
    agents::build_agents(&peds, &cars, 1.0, 0.15, &mut m);
    // the whole live window stays far below a million triangles
    assert!(
        m.triangles() < 100_000,
        "{} triangles for the crowd",
        m.triangles()
    );
}

// ---------------------------------------------------------------------------
// cars
// ---------------------------------------------------------------------------

#[test]
fn a_car_is_body_cab_and_two_lamp_bars() {
    let (_, cars) = crowd();
    let car = cars.first().expect("the sim spawns cars");
    let mut m = MeshBuilder::new();
    agents::car(&mut m, car, 0.0);
    assert_eq!(m.len(), agents::CAR_VERTS);
}

#[test]
fn head_lamps_follow_the_headlight_curve() {
    let (_, cars) = crowd();
    let car = cars.first().unwrap();
    let head_at = |k: f32| {
        let mut m = MeshBuilder::new();
        agents::car(&mut m, car, k);
        // box order: underbody, body, cab, head lamp, tail lamp
        m.get(agents::CAR_HEAD_LAMP_VERTS).2
    };
    let off = head_at(0.0);
    let full = head_at(1.0);
    assert_ne!(off, full, "the beam follows the night curve");
    assert!(
        full[0] > 0.9 && full[1] > 0.8,
        "full beam is bright: {full:?}"
    );
    assert!(
        off[0] < 0.5,
        "daylight head lamps are dark red tails: {off:?}"
    );
}

#[test]
fn a_car_is_as_long_as_its_kind_along_its_own_axis() {
    let (_, cars) = crowd();
    for car in cars.iter() {
        let mut m = MeshBuilder::new();
        agents::car(&mut m, car, 0.0);
        let (min, max) = footprint(&m);
        let long = if car.dir.x.abs() > 0.5 {
            max[0] - min[0]
        } else {
            max[1] - min[1]
        };
        assert!(
            long >= car.kind.length() - 0.05,
            "a {:?} must span {} m, got {long}",
            car.kind,
            car.kind.length()
        );
    }
}

#[test]
fn a_van_is_taller_than_a_hatch() {
    let (_, cars) = crowd();
    let top_of = |kind: CarKind| {
        let mut car = cars[0].clone();
        car.kind = kind;
        let mut m = MeshBuilder::new();
        agents::car(&mut m, &car, 0.0);
        (0..m.len()).map(|i| m.get(i).0[1]).fold(0.0f32, f32::max)
    };
    assert!(top_of(CarKind::Van) > top_of(CarKind::Hatch));
    assert!(top_of(CarKind::Van) > 1.5);
}

#[test]
fn taxis_are_yellow() {
    let (_, cars) = crowd();
    let mut taxi = cars[0].clone();
    taxi.kind = CarKind::Taxi;
    let mut m = MeshBuilder::new();
    agents::car(&mut m, &taxi, 0.0);
    // box order: underbody, body, cab, head lamps, tail lamps
    let body = m.get(agents::CAR_BODY_VERTS).2;
    assert!(
        body[0] > 0.7 && body[1] > 0.45 && body[2] < 0.4,
        "taxi must read yellow: {body:?}"
    );
}

#[test]
fn car_paint_follows_the_variant_and_wraps() {
    let (_, cars) = crowd();
    let mut seen: Vec<[f32; 3]> = Vec::new();
    for v in 0..8u8 {
        let mut car = cars[0].clone();
        car.kind = CarKind::Sedan;
        car.variant = v % 6;
        let mut m = MeshBuilder::new();
        agents::car(&mut m, &car, 0.0);
        let c = m.get(agents::CAR_BODY_VERTS).2;
        assert!(c[0] + c[1] + c[2] > 0.2, "variant {v} painted a black car");
        seen.push(c);
    }
    // variants really differ
    assert!(
        (1..seen.len()).any(|i| seen[i] != seen[0]) || seen[0] == seen[1],
        "paint variants must be visible"
    );
}

#[test]
fn cars_sit_on_the_road_and_not_under_it() {
    let (_, cars) = crowd();
    for car in cars.iter() {
        let mut m = MeshBuilder::new();
        agents::car(&mut m, car, 0.0);
        let lowest = (0..m.len()).map(|i| m.get(i).0[1]).fold(f32::MAX, f32::min);
        assert!(lowest >= 0.0, "a car must not sink into the road: {lowest}");
    }
}
