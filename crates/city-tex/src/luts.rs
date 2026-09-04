//! Small lookup tables that make the material painters cheap and exact.
//!
//! * [`GradientLut`] — a 1D ramp sampled in `0..=1` (used for facade stains, roof
//!   weathering and the grass dryness ramp).
//! * `city-tex` also exposes a "noise LUT" through [`crate::NoiseLut`]; this module
//!   keeps the scalar ramps that are too trivial to deserve their own file.

/// A 1-D colour ramp with `n` entries, evaluated by linear interpolation.
///
/// The two endpoints are extended as constants outside `0..=1`, so a painter can
/// pass an out-of-range coordinate without branching.
#[derive(Clone, Debug, PartialEq)]
pub struct GradientLut {
    stops: Vec<[u8; 3]>,
}

/// Error returned by [`GradientLut::from_stops`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LutError {
    /// A ramp needs at least two stops.
    TooFewStops,
}

impl GradientLut {
    /// Build a ramp from explicit stops (uniformly spaced between the first and last).
    ///
    /// # Errors
    /// [`LutError::TooFewStops`] when fewer than two stops are given.
    pub fn from_stops(stops: &[[u8; 3]]) -> Result<GradientLut, LutError> {
        if stops.len() < 2 {
            return Err(LutError::TooFewStops);
        }
        Ok(GradientLut {
            stops: stops.to_vec(),
        })
    }

    /// A ramp from `a` to `b` sampled at `n` points.
    pub fn linear(a: [u8; 3], b: [u8; 3], n: usize) -> GradientLut {
        let n = n.max(2).max(2);
        let stops = (0..n)
            .map(|i| crate::mix3(a, b, i as f32 / (n - 1) as f32))
            .collect();
        GradientLut { stops }
    }

    /// Number of stored stops.
    pub fn len(&self) -> usize {
        self.stops.len()
    }

    /// `true` when the ramp holds fewer than two stops (it never is, by construction).
    pub fn is_empty(&self) -> bool {
        self.stops.len() < 2
    }

    /// Sample the ramp at `t` in `0..=1` (clamped).
    pub fn sample(&self, t: f32) -> [u8; 3] {
        let t = city_math::saturate(t);
        let n = self.stops.len();
        let x = t * (n - 1) as f32;
        let i = (x as usize).min(n - 1);
        let j = (i + 1).min(n - 1);
        crate::mix3(self.stops[i], self.stops[j], x - i as f32)
    }

    /// The raw stop colours.
    pub fn stops(&self) -> &[[u8; 3]] {
        &self.stops
    }
}

impl Default for GradientLut {
    fn default() -> Self {
        GradientLut::linear([0, 0, 0], [255, 255, 255], 64)
    }
}
