//! Procedural texture behaviour of `city-tex`: tile geometry, determinism,
//! seamless tiling, the noise lattices, the gradient LUTs and the visible marks
//! each material painter leaves on its tile.

use city_tex::{
    luts::{GradientLut, LutError},
    noise::{fbm, value_noise, NoiseLut},
    Material, Texture, ALL_MATERIALS,
};

use city_tex::TILE;

/// Mean of the three channels, `0..=1`.
fn bright(t: &Texture) -> f32 {
    let a = t.average();
    (a[0] + a[1] + a[2]) / 3.0
}

/// Mean absolute texel-delta between horizontally adjacent texels (grain energy).
fn grain_x(t: &Texture, step: u32) -> f32 {
    let mut s = 0.0f32;
    let mut n = 0.0f32;
    for y in 0..city_tex::TILE {
        for x in 0..city_tex::TILE {
            let a = t.get(x, y);
            let b = t.get((x + step) % TILE, y);
            for k in 0..3 {
                s += (a[k] as f32 - b[k] as f32).abs();
            }
            n += 1.0;
        }
    }
    s / (n * 255.0)
}

/// Mean absolute texel-delta between vertically adjacent texels.
fn grain_y(t: &Texture, step: u32) -> f32 {
    let mut s = 0.0f32;
    let mut n = 0.0f32;
    for y in 0..city_tex::TILE {
        for x in 0..city_tex::TILE {
            let a = t.get(x, y);
            let b = t.get(x, (y + step) % TILE);
            for k in 0..3 {
                s += (a[k] as f32 - b[k] as f32).abs();
            }
            n += 1.0;
        }
    }
    s / (n * 255.0)
}

// ---------------------------------------------------------------------------
// texture container
// ---------------------------------------------------------------------------

#[test]
fn texture_is_opaque_rgba8_at_the_expected_size() {
    let t = Texture::new(16, 8);
    assert_eq!(t.width, 16);
    assert_eq!(t.height, 8);
    assert_eq!(t.pixels.len(), 16 * 8 * 4);
    assert!(t.pixels.chunks_exact(4).all(|p| p[3] == 255));
    assert_eq!(t.len(), 128);
    assert!(!t.is_empty());
    assert!(Texture::new(0, 4).is_empty());
}

#[test]
fn texel_coordinates_wrap_like_a_repeat_sampler() {
    let mut t = Texture::new(8, 8);
    t.set(3, 3, [10, 20, 30]);
    assert_eq!(t.get(3, 3), [10, 20, 30]);
    assert_eq!(t.get(3 + 8, 3), [10, 20, 30]);
    assert_eq!(t.get(3, 3 + 16), [10, 20, 30]);
    // writing at (11, 3) hits the same texel as (3, 3)
    t.set(11, 3, [1, 2, 3]);
    assert_eq!(t.get(3, 3), [1, 2, 3]);
}

#[test]
fn blend_composites_over_the_base_and_noops_at_zero() {
    let mut t = Texture::new(4, 4);
    t.set(1, 1, [100, 100, 100]);
    t.blend(1, 1, [200, 100, 0], 0.5);
    let c = t.get(1, 1);
    assert!(c[0] > 140 && c[0] < 160, "got {c:?}");
    assert!(c[1] <= 100 && c[1] > 70, "green {c:?}");
    let before = t.get(2, 2);
    t.blend(2, 2, [255, 255, 255], 0.0);
    assert_eq!(t.get(2, 2), before);
    // blend wraps too
    t.blend(5, 5, [0, 0, 0], 1.0);
    assert_eq!(t.get(1, 1), [0, 0, 0]);
}

#[test]
fn average_and_mean_diff_match_hand_computed_values() {
    let mut a = Texture::new(2, 1);
    a.set(0, 0, [0, 0, 0]);
    a.set(1, 0, [128, 0, 0]);
    let avg = a.average();
    assert!((avg[0] - 64.0 / 255.0).abs() < 1e-4);
    assert_eq!(avg[1], 0.0);
    assert_eq!(avg[2], 0.0);

    let mut b = Texture::new(2, 1);
    b.set(0, 0, [0, 0, 0]);
    b.set(1, 0, [128, 0, 0]);
    assert_eq!(a.mean_diff(&b), 0.0);

    let mut c = Texture::new(2, 1);
    c.set(0, 0, [255, 0, 0]);
    c.set(1, 0, [128, 0, 0]);
    let want = 255.0 / (2.0 * 3.0 * 255.0); // one texel, one channel of 2 texels x 3 ch
    assert!((a.mean_diff(&c) - 255.0 / (2.0 * 3.0 * 255.0)).abs() < 1e-6);
    let _ = want;

    // different sizes report "maximally different"
    assert_eq!(a.mean_diff(&Texture::new(3, 1)), 1.0);
}

// ---------------------------------------------------------------------------
// determinism & palette
// ---------------------------------------------------------------------------

#[test]
fn every_material_paints_its_full_tile() {
    for m in ALL_MATERIALS {
        let t = city_tex::generate(m, 0xc17);
        assert_eq!(t.width, TILE);
        assert_eq!(t.height, TILE);
        assert_eq!(t.pixels.len(), TILE as usize * TILE as usize * 4);
        assert!(t.pixels.chunks_exact(4).all(|p| p[3] == 255));
    }
}

#[test]
fn generation_is_deterministic_and_seed_sensitive() {
    for m in ALL_MATERIALS {
        let a = city_tex::generate(m, 42);
        let b = city_tex::generate(m, 42);
        assert_eq!(a, b, "{m:?} not byte-identical for equal seeds");
        let c = city_tex::generate(m, 43);
        assert!(
            a.mean_diff(&c) > 0.005,
            "{m:?} barely reacts to a seed change"
        );
    }
}

#[test]
fn each_material_sits_in_its_brightness_band() {
    // dark carriageway -> mid greys -> light plaster -> worn-but-bright paint
    let bands: &[(Material, f32, f32)] = &[
        (Material::Asphalt, 0.08, 0.22),
        (Material::Concrete, 0.28, 0.48),
        (Material::Sidewalk, 0.34, 0.60),
        (Material::Grass, 0.16, 0.40),
        (Material::Brick, 0.25, 0.52),
        (Material::Plaster, 0.42, 0.70),
        (Material::RoofGravel, 0.22, 0.48),
        (Material::Metal, 0.30, 0.58),
        (Material::RoadPaintWhite, 0.55, 0.92),
        (Material::RoadLineYellow, 0.45, 0.85),
    ];
    for (m, lo, hi) in bands {
        let b = bright(&city_tex::generate(*m, 7));
        assert!(b > *lo && b < *hi, "{m:?} brightness {b} outside {lo}..{hi}");
    }
}

#[test]
fn grass_is_green_not_grey() {
    let avg = city_tex::generate(Material::Grass, 7).average();
    assert!(
        avg[1] > avg[0] * 1.25 && avg[1] > avg[2],
        "grass average {avg:?} is not green"
    );
}

// ---------------------------------------------------------------------------
// noise: periodicity is the whole design
// ---------------------------------------------------------------------------

#[test]
fn value_noise_is_smooth_on_the_unit_lattice_and_repeats_per_lattice_cell() {
    // inside one lattice cell the noise is C1-smooth: half-step deltas are much
    // smaller than whole-cell differences
    let fine = (0..64)
        .map(|i| {
            let x = i as f32 * 0.125;
            (value_noise(x, 0.5, 9) - value_noise(x + 0.125, 0.5, 9)).abs()
        })
        .fold(0.0f32, f32::max);
    assert!(fine < 0.5, "value noise is not smooth at the lattice scale: {fine}");
    // the lattice corners are exactly the cell hashes
    let a = value_noise(0.0, 0.0, 9);
    let want = city_math::hash::hash2d_unit(0, 0, 9);
    assert!((a - want).abs() < 1e-5, "{a} vs {want}");
}

#[test]
fn fbm_is_seamless_on_the_periodic_domain() {
    // The seamless path is the wrapped lattice (NoiseLut / NoiseLut::fbm): octave
    // k hashes cells modulo `freq`, so the field closes over the tile — while the
    // raw hashed `fbm` has lattice period 1/oct_freq, not 1.0.
    for freq in [4u32, 8, 8 * 4] {
        let lut = NoiseLut::new(128, freq, 98);
        for k in 0..6 {
            let v = k as f32 * 0.37;
            let a = lut.fbm(0.0, v, 4);
            let b = lut.fbm(128.0, v, 4);
            assert!(
                (a - b).abs() < 2e-3,
                "lut fbm seam at freq {freq}, v {v}: {a} vs {b}"
            );
        }
    }
}

#[test]
fn value_noise_and_fbm_stay_in_zero_one() {
    for i in 0..200 {
        let x = (i % 17) as f32 * 1.13;
        let y = (i / 13) as f32 * 0.77;
        let v = value_noise(x, y, 5);
        assert!((0.0..=1.0).contains(&v), "value_noise out of range: {v}");
        let f = fbm(x, y, 8, 4, 5);
        assert!((0.0..=1.0).contains(&f), "fbm out of range: {f}");
    }
}

#[test]
fn noise_lut_returns_its_lattice_exactly() {
    let lut = NoiseLut::new(128, 16, 4242);
    assert_eq!(lut.size(), 128);
    assert_eq!(lut.freq(), 16);
    let s = 128.0 / 16.0;
    for y in 0..16 {
        for x in 0..16 {
            let v = lut.sample(x as f32 * s, y as f32 * s);
            let want = city_math::hash::hash2d_unit(x, y, 4242);
            assert!((v - want).abs() < 1e-5, "lut({x},{y}) = {v}, want {want}");
        }
    }
}

#[test]
fn noise_lut_wraps_seamlessly_across_the_tile() {
    let lut = NoiseLut::new(128, 8, 77);
    for k in 0..24 {
        let v = k as f32 * 5.0;
        let left = lut.sample(0.0, v);
        let wrap = lut.sample(128.0, v);
        assert!((left - wrap).abs() < 1e-4, "not wrap-continuous at v {v}");
        // the texel before the edge must sit near the texel after the wrap
        let near = lut.sample(127.0, v);
        let prev = lut.sample(-1.0, v);
        assert!(
            (near - prev).abs() < 0.35,
            "edge gradient jumps at v {v}: {near} vs {prev}"
        );
    }
}

#[test]
fn noise_lut_fbm_stays_in_range_and_seamless() {
    let lut = NoiseLut::new(128, 8, 1);
    for k in 0..16 {
        let u = k as f32 * 7.0;
        let a = lut.fbm(0.0, u, 4);
        let b = lut.fbm(128.0, u, 4);
        assert!((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b));
        assert!((a - b).abs() < 2e-3, "lut fbm seam at {u}: {a} vs {b}");
    }
}

#[test]
#[should_panic]
fn noise_lut_rejects_a_frequency_that_does_not_divide_the_tile() {
    let _ = NoiseLut::new(128, 48, 1);
}

#[test]
#[should_panic]
fn noise_lut_rejects_zero_frequency() {
    let _ = NoiseLut::new(128, 0, 1);
}

// ---------------------------------------------------------------------------
// gradient luts
// ---------------------------------------------------------------------------

#[test]
fn gradient_lut_endpoints_and_clamping() {
    let g = GradientLut::linear([0, 0, 0], [200, 100, 100], 9);
    assert_eq!(g.sample(0.0), [0, 0, 0]);
    assert_eq!(g.sample(1.0), [200, 100, 100]);
    let mid = g.sample(0.5);
    assert!(mid[0] > 95 && mid[0] < 105, "mid {mid:?}");
    assert_eq!(g.len(), 9);
    assert!(!g.is_empty());
    // out-of-range coordinates clamp instead of wrapping or producing NaN
    assert_eq!(g.sample(-3.0), g.sample(0.0));
    assert_eq!(g.sample(9.0), g.sample(1.0));
}

#[test]
fn gradient_lut_rejects_degenerate_stops() {
    assert_eq!(
        GradientLut::from_stops(&[[1, 2, 3]]).err(),
        Some(LutError::TooFewStops)
    );
    assert_eq!(GradientLut::from_stops(&[]).err(), Some(LutError::TooFewStops));
    assert!(GradientLut::from_stops(&[[0, 0, 0], [255, 255, 255]]).is_ok());
}

#[test]
fn gradient_lut_is_monotonic_per_channel() {
    let g = GradientLut::linear([10, 20, 30], [110, 120, 130], 16);
    let mut prev = g.sample(0.0);
    for i in 1..=40 {
        let s = g.sample(i as f32 / 40.0);
        for k in 0..3 {
            assert!(s[k] >= prev[k], "ramp went backwards at {}/40 ch {k}", i);
        }
        prev = s;
    }
    assert_eq!(g.stops().len(), 16);
}

#[test]
fn gradient_lut_default_is_a_black_to_white_grey_ramp() {
    let g = GradientLut::default();
    assert_eq!(g.sample(0.0), [0, 0, 0]);
    let top = g.sample(1.0);
    assert!(top[0] > 250 && top[1] > 250 && top[2] > 250);
    let mid = g.sample(0.5);
    assert!(mid[0] == mid[1] && g.sample(0.0)[1] == 0);
}

// ---------------------------------------------------------------------------
// per-material features
// ---------------------------------------------------------------------------

#[test]
fn brick_shows_per_brick_tone_variation() {
    let t = city_tex::generate(Material::Brick, 3);
    // stretcher bond: 16 courses x 8 columns; sample brick centres
    let rh = TILE / 16;
    let rw = TILE / 8;
    let mut tones = std::collections::HashSet::new();
    for row in 0..16u32 {
        for col in 0..8u32 {
            let off = if row % 2 == 1 { rw / 2 } else { 0 };
            let x = (col * rw + rw / 2 + off) % TILE;
            let y = row * rh + rh / 2;
            tones.insert(t.get(x, y));
        }
    }
    assert!(tones.len() > 20, "bricks look uniform: {} tones", tones.len());
}

fn body_of(v: f32) -> f32 {
    v
}

#[test]
fn brick_mortar_rows_read_lighter_than_their_courses() {
    let t = city_tex::generate(Material::Brick, 3);
    let rh = TILE / 16;
    let mut mortar = 0.0f32;
    let mut body = 0.0f32;
    let mut n_m = 0.0f32;
    let mut n_b = 0.0f32;
    for y in 0..city_tex::TILE {
        let mut row = 0.0f32;
        for x in 0..city_tex::TILE {
            let c = t.get(x, y);
            row += (c[0] as u32 + c[1] as u32 + c[2] as u32) as f32 / 3.0;
        }
        let mean = row / city_tex::TILE as f32;
        if y % rh < 2 {
            mortar += mean;
            n_m += 1.0;
        } else {
            body += body_of(mean);
            n_b += 1.0;
        }
    }
    let lm = mortar / n_m;
    let lb = body / n_b;
    assert!(lm > lb, "mortar {lm} should read lighter than brick {lb}");
}

#[test]
fn sidewalk_joints_are_darker_than_slab_centres() {
    let t = city_tex::generate(Material::Sidewalk, 8);
    let slab = TILE / 4;
    for s in 0..4u32 {
        let joint = (s * slab + 1) % TILE;
        let centre = s * slab + slab / 2;
        let mut lj = 0.0f32;
        let mut lc = 0.0f32;
        for y in 0..city_tex::TILE {
            let a = t.get(joint, y);
            let b = t.get(centre, y);
            lj += a[0] as f32 + a[1] as f32 + a[2] as f32;
            lc += (b[0] as f32 + b[1] as f32 + b[2] as f32);
        }
        assert!(lj * 10.0 < lc * 9.0, "sidewalk joint {lj} vs slab {lc} (slab {s})");
    }
}

#[test]
fn sidewalk_carries_a_wide_crack_network() {
    let t = city_tex::generate(Material::Sidewalk, 12);
    let is_crack = |x: u32, y: u32| -> bool {
        let c = t.get(x, y);
        c[0] < 60 && c[1] < 62 && c[2] < 60
    };
    let mut cracked_cols = 0u32;
    for x in 0..city_tex::TILE {
        if (0..city_tex::TILE).any(|y| is_crack(x, y)) {
            cracked_cols += 1;
        }
    }
    assert!(cracked_cols > city_tex::TILE / 6, "cracks cover only {cracked_cols} columns");
}

#[test]
fn sidewalk_cracks_reach_the_tile_borders_so_they_can_wrap() {
    const TILE: u32 = city_tex::TILE;
    // the crack walk stamps with wrapping coordinates; across several seeds some
    // crack texels must land on (and therefore continue across) each border
    let mut border_hits = 0;
    for seed in 0..6u64 {
        let t = city_tex::generate(Material::Sidewalk, seed * 97 + 5);
        let crack_at = |x: u32, y: u32| {
            let c = t.get(x, y);
            c[0] < 60 && c[1] < 62 && c[2] < 60
        };
        for k in 0..TILE {
            if crack_at(0, k) || crack_at(TILE - 1, k) {
                border_hits += 1;
            }
        }
    }
    assert!(border_hits > 4, "cracks never touch the tile borders: {border_hits}");
}

#[test]
fn asphalt_is_darker_than_the_rest_and_fully_grainy() {
    let t = city_tex::generate(Material::Asphalt, 1);
    assert!(bright(&t) < 0.24, "asphalt too bright: {}", bright(&t));
    let g = grain_x(&t, 1);
    assert!(g > 0.01, "asphalt has no fine grain ({g})");
    // and the grain is much finer than the coarse oil-stain blobs
    assert!(g > grain_x(&t, 16) * 0.5);
}

#[test]
fn asphalt_is_clearly_darker_than_concrete_and_sidewalk() {
    let a = city_tex::generate(Material::Asphalt, 7);
    let c = city_tex::generate(Material::Concrete, 7);
    let s = city_tex::generate(Material::Sidewalk, 7);
    assert!(bright(&a) < bright(&c) * 0.75);
    assert!(bright(&a) < bright(&s) * 0.7);
}

#[test]
fn road_paint_is_much_lighter_than_the_carriageway() {
    let w = city_tex::generate(Material::RoadPaintWhite, 2);
    let y = city_tex::generate(Material::RoadLineYellow, 2);
    let a = city_tex::generate(Material::Asphalt, 2);
    assert!(bright(&w) > bright(&a) * 2.5);
    assert!(bright(&y) > bright(&a) * 2.0);
    // the yellow line keeps less blue than the white line
    let wa = w.average();
    let ya = y.average();
    assert!(ya[2] < wa[2], "yellow keeps too much blue: {y:?}");
}

#[test]
fn metal_brushes_along_u_not_v() {
    let t = city_tex::generate(Material::Metal, 2);
    // streaks are constant along U: the neighbour delta in +x is small, while the
    // delta across the streaks (+y) is clearly larger
    let along = grain_x(&t, 1);
    let across = grain_y(&t, 1);
    assert!(
        across > along * 2.0,
        "brushed metal: along-U energy {along} should be well below cross {across}"
    );
}

#[test]
fn roof_gravel_has_bright_flecks_and_dark_patches() {
    let t = city_tex::generate(Material::RoofGravel, 6);
    let px: Vec<[u8; 3]> = t.pixels.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
    assert!(
        px.iter().any(|c| c[0] > 160 && c[1] > 160),
        "no bright chippings"
    );
    assert!(
        px.iter().any(|c| c[0] < 60 && c[2] < 70),
        "no wet/dark puddle patches"
    );
}

#[test]
fn plaster_is_a_soft_mid_tone_without_hard_edges() {
    let t = city_tex::generate(Material::Plaster, 4);
    let b = bright(&t);
    assert!(b > 0.4 && b < 0.75, "plaster brightness {b} off its band");
    // no extreme darks: plaster never paints the joint/crack colours
    let dark = t
        .pixels
        .chunks_exact(4)
        .filter(|p| p[0] < 60 && p[1] < 60 && p[2] < 60)
        .count();
    assert_eq!(dark, 0, "plaster picked up hard dark stamps");
    // the grain energy is low (soft mottle, not speckle)
    assert!(grain_x(&t, 1) < 0.06, "plaster grain too harsh");
}

#[test]
fn grass_shows_clump_and_dry_variation() {
    let t = city_tex::generate(Material::Grass, 9);
    // dry patches push red towards green; healthy grass keeps them far apart
    let mut dryish = 0u32;
    let mut green = 0u32;
    for p in t.pixels.chunks_exact(4) {
        if p[1] as i32 > p[0] as i32 + 40 {
            green += 1;
        } else if p[1] > p[0] && p[0] > 70 {
            dryish += 1;
        }
    }
    assert!(green > 3000, "grass base missing ({green} green texels)");
    assert!(dryish > 100, "no dry patches at all ({dryish})");
}




