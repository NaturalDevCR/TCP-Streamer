//! Format normalization: channel mixing and sample-rate conversion so the
//! wire format stays fixed regardless of the capture device.

/// Mixes interleaved `input` with `in_channels` channels into interleaved
/// stereo, replacing `out`. Mono is duplicated; stereo is copied verbatim;
/// for >2 channels the first two channels of each frame (FL/FR) are taken.
/// A trailing partial frame is ignored. `in_channels == 0` clears `out`.
pub fn mix_to_stereo(input: &[f32], in_channels: usize, out: &mut Vec<f32>) {
    out.clear();
    if in_channels == 0 {
        return;
    }

    let frames = input.len() / in_channels;
    out.reserve(frames * 2);

    match in_channels {
        1 => {
            for &sample in input.iter().take(frames) {
                out.extend_from_slice(&[sample, sample]);
            }
        }
        2 => out.extend_from_slice(&input[..frames * 2]),
        _ => {
            for frame in input[..frames * in_channels].chunks_exact(in_channels) {
                out.extend_from_slice(&frame[..2]);
            }
        }
    }
}

/// Largest trim the controller may request, in parts per million. Crystal
/// drift between two machines is typically 10–100 ppm; this leaves headroom
/// without reaching a ratio where Catmull-Rom interpolation becomes audible.
///
/// Declared here because the resampler is what enforces it. `TrimController`
/// imports this rather than declaring its own copy, so the controller's output
/// clamp and the resampler's input clamp cannot drift apart.
pub const MAX_TRIM_PPM: f64 = 200.0;

/// Stateful stereo sample-rate converter (Catmull-Rom cubic interpolation).
/// Accepts arbitrary-length interleaved stereo chunks and keeps interpolation
/// state across calls so chunk boundaries are seamless.
pub struct StereoResampler {
    /// The untrimmed ratio, kept so `set_trim_ppm` is always relative to the
    /// device rates rather than compounding on the previous trim.
    base_step: f64,
    step: f64,
    pos: f64,
    history: [[f32; 2]; 3],
    primed: bool,
    passthrough: bool,
    frames: Vec<[f32; 2]>,
}

impl StereoResampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        Self::build(in_rate, out_rate, in_rate == out_rate)
    }

    /// A resampler that never takes the passthrough shortcut, so its ratio can
    /// be trimmed at any time without a discontinuity.
    ///
    /// `passthrough` is fixed at construction rather than derived from the
    /// current trim on purpose: a resampler that has been running in
    /// passthrough has never primed, so `history` is still zeros. The first
    /// call after the trim went non-zero would interpolate against silence and
    /// emit a click — in the very feature meant to remove clicks.
    pub fn new_adaptive(in_rate: u32, out_rate: u32) -> Self {
        Self::build(in_rate, out_rate, false)
    }

    fn build(in_rate: u32, out_rate: u32, passthrough: bool) -> Self {
        assert!(in_rate > 0, "input sample rate must be non-zero");
        assert!(out_rate > 0, "output sample rate must be non-zero");

        let base_step = in_rate as f64 / out_rate as f64;
        Self {
            base_step,
            step: base_step,
            pos: 0.0,
            history: [[0.0; 2]; 3],
            primed: false,
            passthrough,
            frames: Vec::new(),
        }
    }

    /// Adjusts the resampling ratio by `ppm` parts per million, clamped to
    /// ±[`MAX_TRIM_PPM`]. Interpolation state survives, so the change is
    /// seamless: a positive trim raises `step`, producing fewer output frames
    /// per input frame.
    pub fn set_trim_ppm(&mut self, ppm: f64) {
        let ppm = ppm.clamp(-MAX_TRIM_PPM, MAX_TRIM_PPM);
        self.step = self.base_step * (1.0 + ppm * 1e-6);
    }

    /// True when `process()` copies its input verbatim instead of
    /// interpolating.
    ///
    /// This is a property of the CONSTRUCTOR, not of the rates: only
    /// [`new`](Self::new) sets it, and only when `in_rate == out_rate`.
    /// [`new_adaptive`](Self::new_adaptive) always returns `false`, equal
    /// rates included, because a trimmable resampler must never have an
    /// unprimed fast path to fall out of. Since `SinkPipeline` builds with
    /// `new_adaptive`, its own `is_passthrough` shortcut is unreachable today
    /// — read this before concluding otherwise.
    pub fn is_passthrough(&self) -> bool {
        self.passthrough
    }

    /// Replaces `out` with the resampled stereo frames. Output is always
    /// whole stereo frames (even number of f32). Odd-length input ignores
    /// the trailing sample.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        out.clear();
        let input = &input[..input.len() / 2 * 2];

        if self.passthrough {
            out.extend_from_slice(input);
            return;
        }

        let input_frames = input.len() / 2;
        if input_frames == 0 {
            return;
        }

        if !self.primed {
            let first = [input[0], input[1]];
            self.history = [first; 3];
            self.pos = 3.0;
            self.primed = true;
        }

        self.frames.clear();
        self.frames.reserve(3 + input_frames);
        self.frames.extend_from_slice(&self.history);
        self.frames
            .extend(input.chunks_exact(2).map(|frame| [frame[0], frame[1]]));

        let available = (self.frames.len() as f64 - 2.0 - self.pos).max(0.0);
        out.reserve((available / self.step).ceil() as usize * 2);

        while self.pos.floor() + 2.0 < self.frames.len() as f64 {
            let base = self.pos.floor() as usize;
            let fraction = (self.pos - base as f64) as f32;

            for channel in 0..2 {
                out.push(catmull_rom(
                    self.frames[base - 1][channel],
                    self.frames[base][channel],
                    self.frames[base + 1][channel],
                    self.frames[base + 2][channel],
                    fraction,
                ));
            }

            self.pos += self.step;
        }

        let history_start = self.frames.len() - 3;
        self.history.copy_from_slice(&self.frames[history_start..]);
        self.pos -= input_frames as f64;
    }
}

fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;

    0.5 * (2.0 * p1
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Full sink-side conversion chain: audio received in the source's format
/// (any rate, any channel count) → the playback device's format. Composes
/// [`mix_to_stereo`] → [`StereoResampler`] → [`stereo_to_channels`], with a
/// zero-copy-ish shortcut when no conversion is needed.
pub struct SinkPipeline {
    in_channels: usize,
    out_channels: usize,
    resampler: StereoResampler,
    stereo_in: Vec<f32>,
    stereo_out: Vec<f32>,
}

impl SinkPipeline {
    pub fn new(in_rate: u32, in_channels: u16, out_rate: u32, out_channels: u16) -> Self {
        Self {
            in_channels: in_channels as usize,
            out_channels: out_channels as usize,
            resampler: StereoResampler::new_adaptive(in_rate.max(1), out_rate.max(1)),
            stereo_in: Vec::new(),
            stereo_out: Vec::new(),
        }
    }

    /// Adjusts the resampling ratio to track the source's clock. See
    /// [`StereoResampler::set_trim_ppm`].
    pub fn set_trim_ppm(&mut self, ppm: f64) {
        self.resampler.set_trim_ppm(ppm);
    }

    /// Replaces `out` with `input` converted to the device format. Output is
    /// always whole device frames.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.in_channels == 2 && self.out_channels == 2 && self.resampler.is_passthrough() {
            out.clear();
            out.extend_from_slice(&input[..input.len() / 2 * 2]);
            return;
        }
        mix_to_stereo(input, self.in_channels, &mut self.stereo_in);
        self.resampler
            .process(&self.stereo_in, &mut self.stereo_out);
        stereo_to_channels(&self.stereo_out, self.out_channels, out);
    }
}

/// Spreads interleaved stereo into `out_channels` interleaved channels,
/// replacing `out`: mono gets the L/R average; stereo copies verbatim; with
/// more than two channels L/R land on the front pair and the rest is
/// silence. The inverse direction of [`mix_to_stereo`], used on the sink
/// side where the playback device dictates the channel count.
/// `out_channels == 0` clears `out`. A trailing partial input frame is
/// ignored.
pub fn stereo_to_channels(input: &[f32], out_channels: usize, out: &mut Vec<f32>) {
    out.clear();
    if out_channels == 0 {
        return;
    }
    let frames = input.len() / 2;
    out.reserve(frames * out_channels);
    match out_channels {
        1 => {
            for frame in input[..frames * 2].chunks_exact(2) {
                out.push((frame[0] + frame[1]) * 0.5);
            }
        }
        2 => out.extend_from_slice(&input[..frames * 2]),
        n => {
            for frame in input[..frames * 2].chunks_exact(2) {
                out.push(frame[0]);
                out.push(frame[1]);
                for _ in 2..n {
                    out.push(0.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{mix_to_stereo, StereoResampler, MAX_TRIM_PPM};
    use std::f32::consts::TAU;

    struct Lcg(u32);

    impl Lcg {
        fn new(seed: u32) -> Self {
            Self(seed)
        }

        fn next_u32(&mut self) -> u32 {
            self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            self.0
        }

        fn chunk_frames(&mut self, remaining: usize) -> usize {
            let frames = 16 + self.next_u32() as usize % 2_033;
            frames.min(remaining)
        }
    }

    fn sine_stereo(rate: u32, seconds: f32) -> Vec<f32> {
        let frames = (rate as f32 * seconds) as usize;
        let mut samples = Vec::with_capacity(frames * 2);
        for frame in 0..frames {
            let sample = (TAU * 440.0 * frame as f32 / rate as f32).sin();
            samples.extend_from_slice(&[sample, sample]);
        }
        samples
    }

    fn process_random_chunks(
        resampler: &mut StereoResampler,
        input: &[f32],
        seed: u32,
    ) -> Vec<f32> {
        let mut rng = Lcg::new(seed);
        let mut output = Vec::new();
        let mut chunk_output = Vec::new();
        let mut frame_offset = 0;
        let total_frames = input.len() / 2;

        while frame_offset < total_frames {
            let chunk_frames = rng.chunk_frames(total_frames - frame_offset);
            let start = frame_offset * 2;
            let end = (frame_offset + chunk_frames) * 2;
            resampler.process(&input[start..end], &mut chunk_output);
            output.extend_from_slice(&chunk_output);
            frame_offset += chunk_frames;
        }

        output
    }

    fn count_zero_crossings(samples: &[f32]) -> usize {
        samples
            .windows(2)
            .filter(|pair| (pair[0] <= 0.0 && pair[1] > 0.0) || (pair[0] >= 0.0 && pair[1] < 0.0))
            .count()
    }

    fn assert_resampled_sine(in_rate: u32, out_rate: u32, seed: u32) {
        let seconds = 2.0;
        let input = sine_stereo(in_rate, seconds);
        let input_frames = input.len() / 2;
        let mut resampler = StereoResampler::new(in_rate, out_rate);
        let output = process_random_chunks(&mut resampler, &input, seed);

        assert_eq!(output.len() % 2, 0);
        let output_frames = output.len() / 2;
        let expected_frames = input_frames as f64 * out_rate as f64 / in_rate as f64;
        let frame_error = (output_frames as f64 - expected_frames).abs() / expected_frames;
        assert!(
            frame_error <= 0.001,
            "expected about {expected_frames} frames, got {output_frames}"
        );

        let left: Vec<f32> = output.chunks_exact(2).map(|frame| frame[0]).collect();
        let crossings = count_zero_crossings(&left);
        let expected_crossings = (2.0 * 440.0 * seconds) as usize;
        let crossing_error =
            crossings.abs_diff(expected_crossings) as f64 / expected_crossings as f64;
        assert!(
            crossing_error <= 0.01,
            "expected about {expected_crossings} zero crossings, got {crossings}"
        );
    }

    #[test]
    fn mixes_supported_channel_layouts_to_stereo() {
        let mut output = vec![99.0];

        mix_to_stereo(&[0.25, -0.5, 1.0], 1, &mut output);
        assert_eq!(output, [0.25, 0.25, -0.5, -0.5, 1.0, 1.0]);

        mix_to_stereo(&[0.25, -0.5, 1.0, -1.0], 2, &mut output);
        assert_eq!(output, [0.25, -0.5, 1.0, -1.0]);

        mix_to_stereo(
            &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6],
            6,
            &mut output,
        );
        assert_eq!(output, [0.1, 0.2, 1.1, 1.2]);

        mix_to_stereo(&[0.1, 0.2, 0.3, 0.4, 9.9], 2, &mut output);
        assert_eq!(output, [0.1, 0.2, 0.3, 0.4]);

        mix_to_stereo(&[0.1, 0.2], 0, &mut output);
        assert!(output.is_empty());
    }

    #[test]
    fn equal_rates_pass_through_whole_stereo_frames() {
        let input = [0.0, 0.5, -0.25, 1.0, 0.75, -0.5];
        let mut output = vec![99.0];
        let mut resampler = StereoResampler::new(48_000, 48_000);

        assert!(resampler.is_passthrough());
        resampler.process(&input, &mut output);

        assert_eq!(output, input);
    }

    #[test]
    fn passthrough_ignores_a_trailing_sample() {
        let input = [0.0, 0.5, -0.25, 1.0, 99.0];
        let mut output = Vec::new();
        let mut resampler = StereoResampler::new(48_000, 48_000);

        resampler.process(&input, &mut output);

        assert_eq!(output, input[..4]);
    }

    #[test]
    fn upsamples_44100_to_48000_across_random_chunks() {
        assert_resampled_sine(44_100, 48_000, 0x1234_5678);
    }

    #[test]
    fn downsamples_48000_to_44100_across_random_chunks() {
        assert_resampled_sine(48_000, 44_100, 0x8765_4321);
    }

    #[test]
    fn chunk_boundaries_preserve_ramp_continuity() {
        let input_step = 0.000_01;
        let frames = 24_000;
        let mut input = Vec::with_capacity(frames * 2);
        for frame in 0..frames {
            let sample = frame as f32 * input_step;
            input.extend_from_slice(&[sample, sample]);
        }

        let mut resampler = StereoResampler::new(44_100, 48_000);
        let output = process_random_chunks(&mut resampler, &input, 0x0bad_cafe);
        assert!(!output.is_empty());

        for channel in 0..2 {
            let samples: Vec<f32> = output.chunks_exact(2).map(|frame| frame[channel]).collect();
            for pair in samples.windows(2) {
                let delta = pair[1] - pair[0];
                assert!(
                    delta >= -f32::EPSILON,
                    "channel {channel} decreased from {} to {}",
                    pair[0],
                    pair[1]
                );
                assert!(
                    delta <= input_step * 4.0,
                    "channel {channel} jumped by {delta}"
                );
            }
        }
    }

    #[test]
    fn spreads_stereo_to_device_channel_layouts() {
        use super::stereo_to_channels;
        let mut output = vec![99.0];

        // Mono output: L/R averaged.
        stereo_to_channels(&[0.2, 0.4, -1.0, 1.0], 1, &mut output);
        assert_eq!(output, [0.3, 0.0]);

        // Stereo output: verbatim.
        stereo_to_channels(&[0.1, -0.1, 0.2, -0.2], 2, &mut output);
        assert_eq!(output, [0.1, -0.1, 0.2, -0.2]);

        // 6ch output: front pair carries L/R, the rest silence.
        stereo_to_channels(&[0.5, -0.5], 6, &mut output);
        assert_eq!(output, [0.5, -0.5, 0.0, 0.0, 0.0, 0.0]);

        // Trailing partial frame ignored; zero channels clears.
        stereo_to_channels(&[0.1, 0.2, 9.9], 2, &mut output);
        assert_eq!(output, [0.1, 0.2]);
        stereo_to_channels(&[0.1, 0.2], 0, &mut output);
        assert!(output.is_empty());
    }

    #[test]
    fn sink_pipeline_equal_rate_stereo_is_lossless() {
        use super::SinkPipeline;
        // The sink's resampler is adaptive, so it always interpolates rather
        // than copying. At trim zero that is still sample-exact, but it lags by
        // the interpolator's priming window, so feed enough frames to clear it
        // and compare against the prefix.
        let mut p = SinkPipeline::new(48_000, 2, 48_000, 2);
        let mut input = Vec::new();
        for i in 0..64 {
            input.push(i as f32 * 0.01);
            input.push(i as f32 * -0.01);
        }
        let mut out = Vec::new();
        p.process(&input, &mut out);
        assert!(!out.is_empty());
        assert_eq!(
            out,
            input[..out.len()],
            "equal-rate stereo conversion must stay sample-exact"
        );
    }

    #[test]
    fn sink_pipeline_converts_rate_and_channels_together() {
        use super::SinkPipeline;
        // Source: stereo 44.1k; device: 6ch 48k.
        let mut p = SinkPipeline::new(44_100, 2, 48_000, 6);
        let input = sine_stereo(44_100, 1.0);
        let mut out = Vec::new();
        let mut total_frames = 0usize;
        for chunk in input.chunks(882 * 2) {
            p.process(chunk, &mut out);
            assert_eq!(out.len() % 6, 0, "output must be whole 6ch frames");
            for frame in out.chunks_exact(6) {
                assert_eq!(&frame[2..], &[0.0, 0.0, 0.0, 0.0], "rear channels silent");
            }
            total_frames += out.len() / 6;
        }
        let expected = 44_100.0 * 48_000.0 / 44_100.0; // 1s at 48k
        let err = (total_frames as f64 - expected).abs() / expected;
        assert!(
            err < 0.01,
            "expected ~{expected} frames, got {total_frames}"
        );
    }

    #[test]
    fn sink_pipeline_mono_device_gets_lr_average() {
        use super::SinkPipeline;
        // Same priming caveat as the stereo case. A constant signal keeps the
        // assertion about channel averaging rather than interpolation.
        let mut p = SinkPipeline::new(48_000, 2, 48_000, 1);
        let mut input = Vec::new();
        for _ in 0..64 {
            input.push(0.2);
            input.push(0.4);
        }
        let mut out = Vec::new();
        p.process(&input, &mut out);
        assert!(!out.is_empty());
        for (i, &s) in out.iter().enumerate() {
            assert!(
                (s - 0.3).abs() < 1e-6,
                "sample {i} was {s}, expected the L/R average 0.3"
            );
        }
    }

    #[test]
    fn resampling_preserves_stereo_polarity() {
        let rate = 44_100;
        let frames = rate * 2;
        let mut input = Vec::with_capacity(frames as usize * 2);
        for frame in 0..frames {
            let left = (TAU * 440.0 * frame as f32 / rate as f32).sin();
            input.extend_from_slice(&[left, -left]);
        }

        let mut resampler = StereoResampler::new(rate, 48_000);
        let output = process_random_chunks(&mut resampler, &input, 0xfeed_beef);

        for frame in output.chunks_exact(2) {
            assert!(
                (frame[0] + frame[1]).abs() < 1e-3,
                "stereo mismatch: left={}, right={}",
                frame[0],
                frame[1]
            );
        }
    }

    /// Feeds `chunks` blocks of `frames_per_chunk` stereo frames of a ramp
    /// signal and returns (everything fed, everything produced).
    fn run_chunks(
        r: &mut StereoResampler,
        chunks: usize,
        frames_per_chunk: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let mut fed = Vec::new();
        let mut produced = Vec::new();
        let mut out = Vec::new();
        let mut n = 0.0f32;
        for _ in 0..chunks {
            let mut input = Vec::with_capacity(frames_per_chunk * 2);
            for _ in 0..frames_per_chunk {
                n += 1.0;
                input.push(n);
                input.push(-n);
            }
            r.process(&input, &mut out);
            fed.extend_from_slice(&input);
            produced.extend_from_slice(&out);
        }
        (fed, produced)
    }

    #[test]
    fn trim_zero_is_bit_exact() {
        // The property that justifies enabling the ASRC unconditionally: with
        // step == 1.0 the interpolation position stays integral, fraction is
        // always 0.0, and catmull_rom(.., 0.0) returns p1 exactly. The output
        // is therefore the input stream itself, lagging by the priming frames.
        let mut r = StereoResampler::new_adaptive(48_000, 48_000);
        let (fed, produced) = run_chunks(&mut r, 5, 64);
        assert!(!produced.is_empty());
        assert!(produced.len() <= fed.len());
        assert_eq!(
            produced,
            fed[..produced.len()],
            "trim-zero output must equal the input stream exactly"
        );
    }

    #[test]
    fn positive_trim_yields_fewer_frames() {
        let mut none = StereoResampler::new_adaptive(48_000, 48_000);
        let mut up = StereoResampler::new_adaptive(48_000, 48_000);
        let mut down = StereoResampler::new_adaptive(48_000, 48_000);
        up.set_trim_ppm(100.0);
        down.set_trim_ppm(-100.0);

        let (_, base) = run_chunks(&mut none, 200, 256);
        let (_, faster) = run_chunks(&mut up, 200, 256);
        let (_, slower) = run_chunks(&mut down, 200, 256);

        assert!(
            faster.len() < base.len(),
            "positive trim raises step, so it must emit FEWER frames: {} vs {}",
            faster.len(),
            base.len()
        );
        assert!(
            slower.len() > base.len(),
            "negative trim lowers step, so it must emit MORE frames: {} vs {}",
            slower.len(),
            base.len()
        );
    }

    #[test]
    fn trim_is_clamped() {
        let mut huge = StereoResampler::new_adaptive(48_000, 48_000);
        let mut maxed = StereoResampler::new_adaptive(48_000, 48_000);
        huge.set_trim_ppm(10_000.0);
        maxed.set_trim_ppm(MAX_TRIM_PPM);
        let (_, a) = run_chunks(&mut huge, 50, 256);
        let (_, b) = run_chunks(&mut maxed, 50, 256);
        assert_eq!(
            a.len(),
            b.len(),
            "an over-range trim must behave as the cap"
        );

        let mut tiny = StereoResampler::new_adaptive(48_000, 48_000);
        let mut mined = StereoResampler::new_adaptive(48_000, 48_000);
        tiny.set_trim_ppm(-10_000.0);
        mined.set_trim_ppm(-MAX_TRIM_PPM);
        let (_, c) = run_chunks(&mut tiny, 50, 256);
        let (_, d) = run_chunks(&mut mined, 50, 256);
        assert_eq!(c.len(), d.len());
    }

    #[test]
    fn trim_sweep_introduces_no_discontinuity() {
        // Sweeping the ratio while a smooth signal flows must not produce a
        // step in the output — that step would be the click the ASRC exists
        // to remove.
        let mut r = StereoResampler::new_adaptive(48_000, 48_000);
        let mut out = Vec::new();
        let mut produced: Vec<f32> = Vec::new();
        let mut phase = 0.0f32;
        for chunk in 0..40 {
            // Sweep across the whole legal range, sign included.
            r.set_trim_ppm(MAX_TRIM_PPM * ((chunk as f64 / 20.0) - 1.0));
            let mut input = Vec::with_capacity(512);
            for _ in 0..256 {
                phase += std::f32::consts::TAU * 100.0 / 48_000.0;
                let s = phase.sin();
                input.push(s);
                input.push(s);
            }
            r.process(&input, &mut out);
            produced.extend_from_slice(&out);
        }
        // A 100 Hz sine at 48 kHz moves at most ~0.014 per sample. Allow an
        // order of magnitude of slack for interpolation and the ratio sweep.
        // Bound the collected channel before calling windows(): chaining
        // .collect().windows() on one line borrows from a temporary.
        let left: Vec<f32> = produced.chunks_exact(2).map(|f| f[0]).collect();
        let max_step = left
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            max_step < 0.15,
            "sample-to-sample jump of {max_step} indicates a discontinuity"
        );
    }

    #[test]
    fn new_is_unchanged_by_adaptive_support() {
        // Snapcast regression guard. The source send loop constructs with
        // new(); this is what proves adding the ASRC did not move it.
        assert!(
            StereoResampler::new(48_000, 48_000).is_passthrough(),
            "new() must still short-circuit when rates match"
        );
        assert!(
            !StereoResampler::new_adaptive(48_000, 48_000).is_passthrough(),
            "new_adaptive() must never short-circuit"
        );

        // Output pinned to literals rather than compared against
        // `new_adaptive`: a change that regressed BOTH constructors identically
        // would satisfy a sibling comparison and still have moved Snapcast.
        // These were generated from the current implementation and stand in for
        // pre-branch behaviour; the ramp fed by `run_chunks` makes every one of
        // them a distinct, order-sensitive value.
        let mut plain = StereoResampler::new(44_100, 48_000);
        let (_, a) = run_chunks(&mut plain, 20, 256);
        assert_eq!(a.len(), 11_142, "output length must not move");
        assert_eq!(
            &a[..8],
            &[
                1.0f32,
                -1.0,
                1.915_717_4,
                -1.915_717_4,
                2.8375,
                -2.8375,
                3.756_25,
                -3.756_25
            ],
            "the first frames (priming included) must not move"
        );
        assert_eq!(
            (a[1000], a[1001], a[5000], a[5001]),
            (460.375f32, -460.375, 2297.875, -2297.875),
            "mid-stream interpolation must not move"
        );
        assert_eq!(
            (a[a.len() - 2], a[a.len() - 1]),
            (5118.4375f32, -5118.4375),
            "the accumulated fractional position must not move"
        );

        // Secondary: the adaptive constructor still agrees at trim zero, so the
        // sink and the source see the same stream when nothing is trimming.
        let mut adaptive = StereoResampler::new_adaptive(44_100, 48_000);
        let (_, b) = run_chunks(&mut adaptive, 20, 256);
        assert_eq!(a, b, "at trim zero the two constructors must agree exactly");
    }

    #[test]
    fn adaptive_priming_lag_is_two_frames_once() {
        // Documented, not incidental: the four-tap kernel needs lookahead, so
        // the first call emits two stereo frames fewer than it received. Every
        // later call is 1:1.
        let mut r = StereoResampler::new_adaptive(48_000, 48_000);
        let mut out = Vec::new();
        let input: Vec<f32> = (0..128).map(|i| i as f32).collect(); // 64 frames
        r.process(&input, &mut out);
        assert_eq!(
            out.len(),
            input.len() - 4,
            "first call lags by two stereo frames"
        );
        r.process(&input, &mut out);
        assert_eq!(out.len(), input.len(), "steady state is 1:1");
    }
}
