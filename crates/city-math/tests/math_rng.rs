//! Deterministic RNG: stream stability, ranges, weighted picks, hashing.

use city_math::{Rng, Vec2};

fn stream_of(seed: u64, count: usize) -> Vec<u32> {
    let mut r = Rng::new(seed);
    (0..count).map(|_| r.next_u32()).collect()
}

fn len_of(v: &Vec2) -> f32 {
    v.len()
}

#[test]
fn same_seed_same_stream() {
    let a = stream_of(1234, 64);
    let b = stream_of(1234, 64);
    assert_eq!(a, b, "identical seeds must repeat the same stream");
    let c = stream_of(1235, 64);
    assert_ne!(a, c, "different seeds must diverge");
}

#[test]
fn one_stream_advances() {
    let mut r = Rng::new(7);
    let first: Vec<u32> = (0..16).map(|_| r.next_u32()).collect();
    let second: Vec<u32> = (0..16).map(|_| r.next_u32()).collect();
    assert_ne!(first, second);
}

#[test]
fn f32_stream_is_in_range_and_unbiased() {
    let mut r = Rng::new(99);
    let mut sum = 0.0f32;
    const N: usize = 100_000;
    for _ in 0..N {
        let v = r.next_f32();
        assert!((0.0..1.0).contains(&v), "out of range: {v}");
        sum += v;
    }
    let mean = sum / N as f32;
    assert!((mean - 0.5).abs() < 0.01, "mean of U(0,1) drifted: {mean}");
}

#[test]
fn range_f32_respects_bounds() {
    let mut r = Rng::new(7);
    for _ in 0..5000 {
        let v = r.range_f32(-2.5, 3.5);
        assert!((-2.5..3.5).contains(&v), "{v}");
    }
    assert_eq!(
        r.range_f32(5.0, 5.0),
        5.0,
        "degenerate range collapses to min"
    );
}

#[test]
fn range_i32_is_inclusive_and_degenerate_safe() {
    let mut r = Rng::new(42);
    let mut seen = [false; 7];
    for _ in 0..5000 {
        let v = r.range_i32(3, 9);
        assert!((3..=9).contains(&v), "{v}");
        seen[(v - 3) as usize] = true;
    }
    assert!(seen.iter().all(|s| *s), "every value in range must appear");
    assert_eq!(r.range_i32(4, 4), 4);
    assert_eq!(r.range_i32(9, 1), 9, "inverted range returns min");
}

#[test]
fn index_and_chance() {
    let mut r = Rng::new(5);
    assert_eq!(r.index(0), 0, "n == 0 must not panic");
    for _ in 0..2000 {
        assert!((0..10).contains(&r.index(10)));
    }
    assert!(!r.chance(0.0));
    assert!(r.chance(1.0));
    let mut hits = 0u32;
    for _ in 0..20_000 {
        if r.chance(0.25) {
            hits += 1;
        }
    }
    let ratio = hits as f32 / 20_000.0;
    assert!((ratio - 0.25).abs() < 0.02, "chance(0.25) ratio={ratio}");
}

#[test]
fn weighted_picks_respect_weights() {
    let mut r = Rng::new(2024);
    let w = [1.0f32, 3.0, 0.0];
    let mut counts = [0u32; 3];
    for _ in 0..20_000 {
        counts[r.weighted(&w)] += 1;
    }
    assert_eq!(counts[2], 0, "zero weight must never be picked");
    assert!(
        counts[1] > counts[0] * 2,
        "3x weight must dominate: {counts:?}"
    );
    assert_eq!(r.weighted(&[]), 0);
    assert_eq!(r.weighted(&[0.0, 0.0]), 0);
}

#[test]
fn direction2_produces_unit_vectors() {
    let mut r = Rng::new(31);
    for _ in 0..1000 {
        let d = r.direction2();
        assert!((len_of(&d) - 1.0).abs() < 1e-5, "{d:?}");
    }
}

#[test]
fn u64_output_uses_the_full_width() {
    let mut r = Rng::new(8);
    let mut wide = false;
    for _ in 0..500 {
        let v = r.next_u64();
        wide |= v > u32::MAX as u64;
    }
    assert!(wide, "u64 stream must exceed 32 bit range");
}

#[test]
fn fork_and_reseed_are_reproducible() {
    let mut base = Rng::new(777);
    for _ in 0..10 {
        base.next_u32();
    }
    let mut f = base.fork();
    let forked: Vec<u32> = (0..8).map(|_| f.next_u32()).collect();
    let direct: Vec<u32> = (0..8).map(|_| base.next_u32()).collect();
    assert_eq!(forked, direct, "a fork replays the parent stream");

    let mut r2 = Rng::new(1);
    r2.reseed(777);
    assert_eq!(r2.next_u32(), Rng::new(777).next_u32());
}

#[test]
fn hash_helpers_are_stable_and_well_spread() {
    assert_eq!(city_math::mix(1), city_math::mix(1));
    assert_ne!(city_math::mix(1), city_math::mix(2));
    assert_ne!(city_math::hash12(1, 2), city_math::hash12(2, 1));
    assert_eq!(
        city_math::hash2d(3, -4, 7),
        city_math::hash2d(3, -4, 7),
        "hash2d must be pure"
    );
    assert_ne!(city_math::hash2d(1, 1, 1), city_math::hash2d(1, 1, 2));

    let mut set = std::collections::HashSet::new();
    for x in 0..64i32 {
        for y in 0..64i32 {
            set.insert(city_math::hash2d(x, y, 3));
        }
    }
    assert_eq!(set.len(), 64 * 64, "hash2d collided on a 64x64 lattice");

    for x in 0..32i32 {
        for y in 0..32i32 {
            let u = city_math::hash::hash2d_unit(x, y, 11);
            assert!((0.0..=1.0).contains(&u), "{u}");
        }
    }
}

#[test]
fn world_to_cell_quantises() {
    assert_eq!(city_math::hash::world_to_cell(0.0, 8.0), 0);
    assert_eq!(city_math::hash::world_to_cell(7.9, 8.0), 0);
    assert_eq!(city_math::hash::world_to_cell(8.1, 8.0), 1);
    assert_eq!(city_math::hash::world_to_cell(-0.1, 8.0), -1);
    assert_eq!(
        city_math::hash::world_to_cell(100.0, 0.0),
        0,
        "bad cell size is safe"
    );
}

#[test]
fn hash2d_unit_decorrelates_neighbouring_cells() {
    // Value noise over this lattice is only a noise field if neighbouring lattice
    // cells are uncorrelated: measure the lag-1 correlation over a 128x128 patch.
    let mut a_vals = Vec::new();
    let mut b_vals = Vec::new();
    for x in 0..128i32 {
        for y in 0..128i32 {
            a_vals.push(city_math::hash::hash2d_unit(x, y, 5) as f64);
            b_vals.push(city_math::hash::hash2d_unit(x + 1, y, 5) as f64);
        }
    }
    let mean_a: f64 = a_vals.iter().sum::<f64>() / a_vals.len() as f64;
    let mean_b: f64 = b_vals.iter().sum::<f64>() / b_vals.len() as f64;
    let cov: f64 = a_vals
        .iter()
        .zip(&b_vals)
        .map(|(a, b)| (a - mean_a) * (b - mean_a))
        .sum::<f64>()
        / a_vals.len() as f64;
    let var_a: f64 = a_vals.iter().map(|v| (v - mean_a).powi(2)).sum::<f64>() / a_vals.len() as f64;
    let var_b: f64 = b_vals.iter().map(|v| (v - mean_b).powi(2)).sum::<f64>() / b_vals.len() as f64;
    assert!(
        cov.abs() < 0.02 * (var_a * var_b).sqrt(),
        "hash2d_unit neighbours correlate: cov {cov}, sd {} {}",
        var_a.sqrt(),
        var_b.sqrt()
    );
}
