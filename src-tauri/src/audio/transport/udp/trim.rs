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

// Dimensionless pole-placement numerators. The gains themselves depend on the
// plant, which is only known at construction, so what is constant here is the
// numerator; `new` divides by the normalized loop gain `g`.
//
// Plant: occupancy integrates the uncancelled clock error, so one tick of
// residual `r` ppm adds `samples_per_tick * r * 1e-6` samples, i.e.
// `g * r` in normalized units where `g = samples_per_tick * 1e-6 / target`.
// The sample rate and channel count cancel out of `g`: at a 100 ms tick it is
// exactly `1e-4 / target_seconds`.
//
// With `update`'s ordering (the current error is accumulated into the integral
// *before* the trim is formed), the closed-loop state `(error, integral_prev)`
// advances by
//
//     A = [[1 - g*(KP + KI), -g*KI],
//          [              1,     1]]
//
// so `tr(A) = 2 - g*(KP + KI)` and `det(A) = 1 - g*KP`. Placing both poles at
// damping ZETA and natural frequency `omega_n` rad/tick gives
//
//     r   = exp(-ZETA * omega_n)
//     det = r^2
//     tr  = 2 * r * cos(omega_n * sqrt(1 - ZETA^2))
//
// and therefore `KP = (1 - det) / g` and `KI = (det - tr + 1) / g`.
//
// (A PI controller that instead sampled the integral *before* accumulating
// would give `tr = 2 - g*KP`, i.e. `KP = (2 - tr) / g`; that is the same loop
// with the proportional path renamed to `KP + KI`, and the two differ here by
// 0.2%.)
//
// The poles are placed at damping ZETA = 0.9 — nearly critical, because clock
// drift moves with temperature so there is nothing to chase quickly, and
// overshoot costs audio quality: the interpolation artifact scales with |trim|,
// so a loop that overshoots to a large trim is audibly worse than one that
// creeps — and at a decay of ZETA * omega_n = 4/1200 nepers per tick, i.e. a
// ~120 s settling time at the caller's 10 Hz cadence. Evaluating there gives
//   r = 0.996672216…, det = 0.993355506…, tr = 1.993341834…
// `pole_placement_constants_match_their_derivation` re-runs that arithmetic.
const KP_NUM: f64 = 0.006_644_493_744_965_674;
const KI_NUM: f64 = 0.000_013_671_782_200_574_967;

pub struct TrimController {
    target: f32,
    kp: f64,
    ki: f64,
    integral: f64,
    integral_limit: f64,
    trim_ppm: f64,
}

impl TrimController {
    /// `target` is the standing occupancy to hold, in the same unit later
    /// passed to [`update`](Self::update) (device samples).
    ///
    /// `samples_per_tick` is how many device samples flow through the sink in
    /// one control tick (`out_rate * channels * tick_seconds`). Together with
    /// `target` it *is* the plant, and the gains are derived from it — gains
    /// fixed independently of the plant were off by ~520x, which left the loop
    /// essentially undamped.
    pub fn new(target: f32, samples_per_tick: f64) -> Self {
        // A zero or negative target would make the normalized error
        // undefined; the floor keeps the loop finite for a degenerate
        // configuration instead of producing NaN in the audio path.
        let target = target.max(1.0);
        let g = samples_per_tick * 1e-6 / target as f64;
        // A degenerate plant (no flow, or a non-finite rate) carries no
        // information about what trim would help, and dividing by it would put
        // an infinity into the audio path. Hold the trim at zero instead.
        let (kp, ki) = if g.is_finite() && g > 0.0 {
            (KP_NUM / g, KI_NUM / g)
        } else {
            (0.0, 0.0)
        };
        Self {
            target,
            kp,
            ki,
            integral: 0.0,
            // Anti-windup: the integral term alone can never exceed the output
            // clamp. With the loop disabled there is nothing to wind up.
            integral_limit: if ki > 0.0 { MAX_TRIM_PPM / ki } else { 0.0 },
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
        self.trim_ppm =
            (self.kp * error + self.ki * self.integral).clamp(-MAX_TRIM_PPM, MAX_TRIM_PPM);
        self.trim_ppm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Broadcast default plant: 48 kHz stereo, 250 ms of standing
    /// occupancy, ticked at 10 Hz.
    const OUT_RATE: f64 = 48_000.0;
    const CH: f64 = 2.0;
    const TICK_S: f64 = 0.1;
    const SAMPLES_PER_TICK: f64 = OUT_RATE * CH * TICK_S;
    const TARGET: f32 = (48_000 * 2 * 250 / 1000) as f32;

    /// Minimal plant: occupancy drifts by the clock mismatch the controller has
    /// not yet cancelled. `k` converts residual ppm into samples per tick, and
    /// is derived from the same quantities the constructor takes so the model
    /// and the implementation cannot disagree — a hand-picked `k` here is what
    /// hid gains validated against a plant ~520x too stiff.
    fn simulate(disturbance_ppm: f64, ticks: usize) -> (f64, f32) {
        let mut c = TrimController::new(TARGET, SAMPLES_PER_TICK);
        let mut occupancy = TARGET;
        let k = SAMPLES_PER_TICK * 1e-6;
        for _ in 0..ticks {
            let trim = c.update(occupancy);
            occupancy += ((disturbance_ppm - trim) * k) as f32;
        }
        (c.trim_ppm, occupancy)
    }

    /// Target closed-loop damping ratio, and the pole decay rate in nepers per
    /// tick (`ZETA * omega_n`, for a ~120 s settling time at 10 Hz).
    const ZETA: f64 = 0.9;
    const DECAY_PER_TICK: f64 = 4.0 / 1200.0;

    #[test]
    fn pole_placement_constants_match_their_derivation() {
        // KP_NUM/KI_NUM are literals because `exp` and `cos` are not const, so
        // the derivation lives here instead of in the compiler. Without this,
        // the comment above them is unchecked prose.
        let omega_n = DECAY_PER_TICK / ZETA;
        let r = (-DECAY_PER_TICK).exp();
        let det = r * r;
        let tr = 2.0 * r * (omega_n * (1.0 - ZETA * ZETA).sqrt()).cos();
        assert!(
            (KP_NUM - (1.0 - det)).abs() < 1e-15,
            "KP_NUM {KP_NUM} should be 1 - det = {}",
            1.0 - det
        );
        assert!(
            (KI_NUM - (det - tr + 1.0)).abs() < 1e-18,
            "KI_NUM {KI_NUM} should be det - tr + 1 = {}",
            det - tr + 1.0
        );
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
                (occupancy - TARGET).abs() < TARGET * 0.05,
                "occupancy {occupancy} should return to target"
            );
        }
    }

    #[test]
    fn settles_within_three_minutes_without_a_large_excursion() {
        // The bound that makes the gains a requirement rather than a comment.
        // A loop that hunts is worse than the drop/insert it replaced, so this
        // pins settling time AND the excursion taken to get there: undamped
        // gains satisfy neither, whatever their steady state eventually is.
        for disturbance in [50.0f64, -50.0] {
            let mut c = TrimController::new(TARGET, SAMPLES_PER_TICK);
            let mut occupancy = TARGET;
            let k = SAMPLES_PER_TICK * 1e-6;
            let mut worst_deviation = 0.0f32;
            let mut trim_at_600 = 0.0f64;
            for tick in 0..1800 {
                let trim = c.update(occupancy);
                occupancy += ((disturbance - trim) * k) as f32;
                worst_deviation = worst_deviation.max((occupancy - TARGET).abs());
                if tick == 599 {
                    trim_at_600 = trim;
                }
            }
            // Checkpoint on the SHAPE of the response, not just its endpoint.
            // Tick 600 (60 s) is where a correctly-damped loop and an undamped
            // one diverge most clearly, well before either reaches the 180 s
            // endpoint below: a correctly-damped loop is mid-transient there
            // (measured ~7.7 ppm from the disturbance), while the pre-fix,
            // essentially undamped gains are still only barely off zero
            // (~42.5 ppm from the disturbance) because their ~12-minute hunt
            // period hasn't turned yet. The endpoint bound alone let those
            // broken gains slip through at 2.35 ppm against the 2 ppm bound
            // below, purely because 1800 ticks happened to land near a
            // favorable point in that slow hunt.
            assert!(
                (trim_at_600 - disturbance).abs() < 20.0,
                "after 60 s the trim {trim_at_600} should already be within 20 ppm of the \
                 {disturbance} ppm offset"
            );
            assert!(
                (c.trim_ppm - disturbance).abs() < 2.0,
                "after 180 s the trim {} should be within 2 ppm of the {disturbance} ppm offset",
                c.trim_ppm
            );
            assert!(
                (occupancy - TARGET).abs() < TARGET * 0.02,
                "after 180 s occupancy {occupancy} should be within 2% of target"
            );
            assert!(
                worst_deviation < TARGET * 0.10,
                "occupancy strayed {worst_deviation} samples from target \
                 (>10%) while settling on {disturbance} ppm"
            );
        }
    }

    #[test]
    fn sign_is_correct() {
        // A sign error drives the buffer to a rail instead of failing loudly,
        // so this is asserted directly rather than inferred from convergence.
        let mut over = TrimController::new(1000.0, SAMPLES_PER_TICK);
        assert!(
            over.update(1200.0) > 0.0,
            "occupancy above target must raise step (positive trim) to emit fewer frames"
        );

        let mut under = TrimController::new(1000.0, SAMPLES_PER_TICK);
        assert!(
            under.update(800.0) < 0.0,
            "occupancy below target must lower step (negative trim) to emit more frames"
        );
    }

    #[test]
    fn on_target_is_neutral() {
        let mut c = TrimController::new(1000.0, SAMPLES_PER_TICK);
        for _ in 0..50 {
            assert_eq!(c.update(1000.0), 0.0);
        }
    }

    #[test]
    fn output_is_clamped() {
        let mut c = TrimController::new(1000.0, SAMPLES_PER_TICK);
        for _ in 0..10_000 {
            c.update(50_000.0);
        }
        assert_eq!(c.trim_ppm, MAX_TRIM_PPM);

        let mut c = TrimController::new(1000.0, SAMPLES_PER_TICK);
        for _ in 0..10_000 {
            c.update(0.0);
        }
        assert_eq!(c.trim_ppm, -MAX_TRIM_PPM);
    }

    #[test]
    fn integral_does_not_wind_up() {
        // After a long saturated excursion, returning to target must bring the
        // trim back through zero within a bounded number of ticks rather than
        // staying railed.
        let mut c = TrimController::new(1000.0, SAMPLES_PER_TICK);
        for _ in 0..10_000 {
            c.update(50_000.0);
        }
        assert_eq!(c.trim_ppm, MAX_TRIM_PPM);

        let mut ticks = 0;
        while c.trim_ppm > 0.0 && ticks < 500 {
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
        let mut c = TrimController::new(0.0, SAMPLES_PER_TICK);
        let trim = c.update(0.0);
        assert!(trim.is_finite(), "trim must stay finite for a zero target");
    }

    #[test]
    fn degenerate_plant_holds_the_trim_at_zero() {
        // `samples_per_tick` comes from a negotiated device rate. If that ever
        // arrives as zero or non-finite, dividing by it would put an infinity
        // into the resampler's step; the loop disables itself instead.
        for spt in [0.0f64, -1.0, f64::NAN, f64::INFINITY] {
            let mut c = TrimController::new(TARGET, spt);
            for occupancy in [0.0f32, TARGET, TARGET * 10.0] {
                let trim = c.update(occupancy);
                assert_eq!(trim, 0.0, "degenerate plant {spt} must produce no trim");
            }
        }
    }

    #[test]
    fn gains_scale_with_the_plant() {
        // The normalized loop gain is `1e-4 / target_seconds` regardless of
        // sample rate or channel count, so a 96 kHz 4-channel sink at the same
        // latency must get the same normalized loop — i.e. gains that differ
        // by exactly the ratio of the plants.
        let stereo_48 = TrimController::new(TARGET, SAMPLES_PER_TICK);
        let quad_96 = TrimController::new((96_000 * 4 * 250 / 1000) as f32, 96_000.0 * 4.0 * 0.1);
        let ratio = quad_96.kp / stereo_48.kp;
        assert!(
            (ratio - 1.0).abs() < 1e-9,
            "normalized gains must not depend on rate or channel count (ratio {ratio})"
        );
        assert!((quad_96.ki / stereo_48.ki - 1.0).abs() < 1e-9);
    }
}
