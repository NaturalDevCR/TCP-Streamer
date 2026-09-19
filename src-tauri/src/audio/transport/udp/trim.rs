//! Continuous clock-drift controller (PI on playback-buffer occupancy).
//!
//! The source's capture clock and the sink's playback clock run independently.
//! Rather than dropping or inserting samples when they diverge — which is
//! audible — this nudges the sink's resampling ratio by a few parts per
//! million so the difference is absorbed continuously.
//!
//! The integral term converges to whatever trim cancels the clock difference,
//! so nothing has to estimate the ratio: the loop finds it. Occupancy is a
//! noisy instantaneous signal because of network jitter, and the loop is
//! deliberately slow to match — clock ratios move with temperature, not by the
//! second.
//!
//! Pure: no I/O and no clock of its own. The caller decides the cadence.

use crate::audio::engine::convert::MAX_TRIM_PPM;

/// Proportional gain. A 10% occupancy error produces a 4 ppm nudge.
const KP: f64 = 40.0;
/// Integral gain. Accumulates the standing offset over tens of seconds.
const KI: f64 = 2.0;

pub struct TrimController {
    target: f32,
    integral: f64,
    integral_limit: f64,
    trim_ppm: f64,
}

impl TrimController {
    /// `target` is the standing occupancy to hold, in the same unit later
    /// passed to [`update`](Self::update) (device samples).
    pub fn new(target: f32) -> Self {
        Self {
            // A zero or negative target would make the normalized error
            // undefined; the floor keeps the loop finite for a degenerate
            // configuration instead of producing NaN in the audio path.
            target: target.max(1.0),
            integral: 0.0,
            integral_limit: MAX_TRIM_PPM / KI,
            trim_ppm: 0.0,
        }
    }

    /// Advances one control tick and returns the trim to apply, in ppm.
    ///
    /// Sign: occupancy above target means the source is producing faster than
    /// the sink consumes, so the resampler must emit FEWER frames — which means
    /// a LARGER step, hence a positive trim.
    pub fn update(&mut self, occupancy: f32) -> f64 {
        let error = ((occupancy - self.target) / self.target) as f64;
        self.integral = (self.integral + error).clamp(-self.integral_limit, self.integral_limit);
        self.trim_ppm = (KP * error + KI * self.integral).clamp(-MAX_TRIM_PPM, MAX_TRIM_PPM);
        self.trim_ppm
    }

    /// The trim most recently returned by [`update`](Self::update).
    ///
    /// Exposed for introspection (tests exercise convergence and clamping
    /// through this getter); `receive_loop` only needs the return value of
    /// `update` itself, so this has no production caller yet.
    #[allow(dead_code)]
    pub fn trim_ppm(&self) -> f64 {
        self.trim_ppm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal plant: occupancy drifts by the clock mismatch the controller has
    /// not yet cancelled. `k` converts residual ppm into samples per tick.
    fn simulate(disturbance_ppm: f64, ticks: usize) -> (f64, f32) {
        let target = 9600.0f32;
        let mut c = TrimController::new(target);
        let mut occupancy = target;
        let k = 2.0f64;
        for _ in 0..ticks {
            let trim = c.update(occupancy);
            occupancy += ((disturbance_ppm - trim) * k) as f32;
        }
        (c.trim_ppm(), occupancy)
    }

    #[test]
    fn converges_against_a_simulated_clock_offset() {
        // Convergence is a closed-loop property. At a CONSTANT occupancy the
        // integral term correctly winds to its limit, so an open-loop test
        // would assert the opposite of the real requirement.
        for disturbance in [25.0f64, -25.0, 80.0, -80.0] {
            let (trim, occupancy) = simulate(disturbance, 4000);
            assert!(
                (trim - disturbance).abs() < 5.0,
                "trim {trim} should converge toward the {disturbance} ppm offset"
            );
            assert!(
                (occupancy - 9600.0).abs() < 9600.0 * 0.05,
                "occupancy {occupancy} should return to target"
            );
        }
    }

    #[test]
    fn sign_is_correct() {
        // A sign error drives the buffer to a rail instead of failing loudly,
        // so this is asserted directly rather than inferred from convergence.
        let mut over = TrimController::new(1000.0);
        assert!(
            over.update(1200.0) > 0.0,
            "occupancy above target must raise step (positive trim) to emit fewer frames"
        );

        let mut under = TrimController::new(1000.0);
        assert!(
            under.update(800.0) < 0.0,
            "occupancy below target must lower step (negative trim) to emit more frames"
        );
    }

    #[test]
    fn on_target_is_neutral() {
        let mut c = TrimController::new(1000.0);
        for _ in 0..50 {
            assert_eq!(c.update(1000.0), 0.0);
        }
    }

    #[test]
    fn output_is_clamped() {
        let mut c = TrimController::new(1000.0);
        for _ in 0..10_000 {
            c.update(50_000.0);
        }
        assert_eq!(c.trim_ppm(), MAX_TRIM_PPM);

        let mut c = TrimController::new(1000.0);
        for _ in 0..10_000 {
            c.update(0.0);
        }
        assert_eq!(c.trim_ppm(), -MAX_TRIM_PPM);
    }

    #[test]
    fn integral_does_not_wind_up() {
        // After a long saturated excursion, returning to target must bring the
        // trim back through zero within a bounded number of ticks rather than
        // staying railed.
        let mut c = TrimController::new(1000.0);
        for _ in 0..10_000 {
            c.update(50_000.0);
        }
        assert_eq!(c.trim_ppm(), MAX_TRIM_PPM);

        let mut ticks = 0;
        while c.trim_ppm() > 0.0 && ticks < 500 {
            c.update(0.0);
            ticks += 1;
        }
        assert!(
            ticks < 500,
            "trim stayed railed for {ticks} ticks after the excursion cleared"
        );
    }

    #[test]
    fn zero_target_does_not_divide_by_zero() {
        let mut c = TrimController::new(0.0);
        let trim = c.update(0.0);
        assert!(trim.is_finite(), "trim must stay finite for a zero target");
    }
}
