//! Spectral morphing between two timbres.
//!
//! Analyses audio as a set of harmonic partials, interpolates between two
//! spectral frames, and re-synthesises the result via additive synthesis.

use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// SpectralFrame
// ---------------------------------------------------------------------------

/// A snapshot of a sound's spectral content as a set of partial descriptors.
#[derive(Debug, Clone)]
pub struct SpectralFrame {
    /// Frequencies of each partial in Hz.
    pub frequencies: Vec<f64>,
    /// Amplitude (linear gain) of each partial, in [0.0, 1.0].
    pub amplitudes: Vec<f64>,
    /// Phase of each partial in radians.
    pub phases: Vec<f64>,
}

impl SpectralFrame {
    /// Create a new frame, validating that all slices have equal length.
    pub fn new(frequencies: Vec<f64>, amplitudes: Vec<f64>, phases: Vec<f64>) -> Self {
        assert_eq!(
            frequencies.len(),
            amplitudes.len(),
            "frequencies and amplitudes must have equal length"
        );
        assert_eq!(
            frequencies.len(),
            phases.len(),
            "frequencies and phases must have equal length"
        );
        SpectralFrame {
            frequencies,
            amplitudes,
            phases,
        }
    }

    /// Number of partials in this frame.
    pub fn n_partials(&self) -> usize {
        self.frequencies.len()
    }
}

// ---------------------------------------------------------------------------
// Spectrum analysis
// ---------------------------------------------------------------------------

/// Extract `n_partials` harmonic partials from a sample buffer.
///
/// Each partial is located at `fundamental_hz * k` for k = 1..=n_partials.
/// Amplitude is estimated by averaging the magnitude of a short window around
/// the expected partial frequency bin (no FFT dependency — pure sinusoidal
/// matching via DFT of a short segment).
///
/// Phase is estimated from the real/imaginary components at the partial bin.
pub fn analyze_spectrum(
    samples: &[f64],
    n_partials: usize,
    fundamental_hz: f64,
) -> SpectralFrame {
    // Assume 44100 Hz sample rate for bin calculation.
    let sample_rate = 44100.0_f64;
    let n = samples.len();

    let mut frequencies = Vec::with_capacity(n_partials);
    let mut amplitudes = Vec::with_capacity(n_partials);
    let mut phases = Vec::with_capacity(n_partials);

    for k in 1..=(n_partials as u64) {
        let freq = fundamental_hz * k as f64;
        frequencies.push(freq);

        // DFT at this single frequency.
        let omega = 2.0 * PI * freq / sample_rate;
        let mut re = 0.0_f64;
        let mut im = 0.0_f64;
        for (i, &s) in samples.iter().enumerate() {
            re += s * (omega * i as f64).cos();
            im -= s * (omega * i as f64).sin();
        }
        let mag = (re * re + im * im).sqrt() / n.max(1) as f64;
        let phase = im.atan2(re);

        amplitudes.push(mag.min(1.0));
        phases.push(phase);
    }

    SpectralFrame::new(frequencies, amplitudes, phases)
}

// ---------------------------------------------------------------------------
// Morphing
// ---------------------------------------------------------------------------

/// Linearly interpolate between two spectral frames.
///
/// `alpha` = 0.0 → `frame_a`, `alpha` = 1.0 → `frame_b`.
/// Both frames must have the same number of partials.
pub fn morph_frames(frame_a: &SpectralFrame, frame_b: &SpectralFrame, alpha: f64) -> SpectralFrame {
    let alpha = alpha.clamp(0.0, 1.0);
    let n = frame_a.n_partials().min(frame_b.n_partials());

    let mut frequencies = Vec::with_capacity(n);
    let mut amplitudes = Vec::with_capacity(n);
    let mut phases = Vec::with_capacity(n);

    for i in 0..n {
        frequencies.push(frame_a.frequencies[i] * (1.0 - alpha) + frame_b.frequencies[i] * alpha);
        amplitudes.push(frame_a.amplitudes[i] * (1.0 - alpha) + frame_b.amplitudes[i] * alpha);

        // Interpolate phase with shortest-path wrapping.
        let mut dp = frame_b.phases[i] - frame_a.phases[i];
        while dp > PI {
            dp -= 2.0 * PI;
        }
        while dp < -PI {
            dp += 2.0 * PI;
        }
        phases.push(frame_a.phases[i] + alpha * dp);
    }

    SpectralFrame::new(frequencies, amplitudes, phases)
}

// ---------------------------------------------------------------------------
// Synthesis
// ---------------------------------------------------------------------------

/// Additively synthesise audio samples from a spectral frame.
pub fn synthesize_frame(
    frame: &SpectralFrame,
    duration_samples: usize,
    sample_rate: u32,
) -> Vec<f64> {
    let sr = sample_rate as f64;
    let mut output = vec![0.0_f64; duration_samples];

    for i in 0..frame.n_partials() {
        let freq = frame.frequencies[i];
        let amp = frame.amplitudes[i];
        let phase = frame.phases[i];
        let omega = 2.0 * PI * freq / sr;
        for (t, sample) in output.iter_mut().enumerate() {
            *sample += amp * (omega * t as f64 + phase).sin();
        }
    }

    // Normalise to prevent clipping.
    let peak = output.iter().fold(0.0_f64, |m, &s| m.max(s.abs()));
    if peak > 1.0 {
        output.iter_mut().for_each(|s| *s /= peak);
    }

    output
}

// ---------------------------------------------------------------------------
// SpectralMorpher
// ---------------------------------------------------------------------------

pub struct SpectralMorpher;

impl SpectralMorpher {
    pub fn new() -> Self {
        SpectralMorpher
    }

    /// Generate a morph sequence from `source` to `target` in `steps` steps.
    ///
    /// Each step contributes `samples_per_step` audio samples to the output.
    pub fn morph_sequence(
        &self,
        source: &SpectralFrame,
        target: &SpectralFrame,
        steps: usize,
        sample_rate: u32,
        samples_per_step: usize,
    ) -> Vec<f64> {
        if steps == 0 {
            return vec![];
        }
        let mut output = Vec::with_capacity(steps * samples_per_step);
        for step in 0..steps {
            let alpha = step as f64 / (steps - 1).max(1) as f64;
            let frame = morph_frames(source, target, alpha);
            let chunk = synthesize_frame(&frame, samples_per_step, sample_rate);
            output.extend_from_slice(&chunk);
        }
        output
    }

    /// Cross-synthesis: carrier frequencies + modulator amplitudes.
    pub fn cross_synthesis(
        &self,
        carrier: &SpectralFrame,
        modulator: &SpectralFrame,
    ) -> SpectralFrame {
        let n = carrier.n_partials().min(modulator.n_partials());
        let frequencies = carrier.frequencies[..n].to_vec();
        let amplitudes = modulator.amplitudes[..n].to_vec();
        let phases = carrier.phases[..n].to_vec();
        SpectralFrame::new(frequencies, amplitudes, phases)
    }
}

impl Default for SpectralMorpher {
    fn default() -> Self {
        SpectralMorpher::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(n: usize, base_freq: f64) -> SpectralFrame {
        let frequencies: Vec<f64> = (1..=n).map(|k| base_freq * k as f64).collect();
        let amplitudes: Vec<f64> = (1..=n).map(|k| 1.0 / k as f64).collect();
        let phases: Vec<f64> = vec![0.0; n];
        SpectralFrame::new(frequencies, amplitudes, phases)
    }

    #[test]
    fn spectral_frame_new_panics_on_mismatch() {
        let result = std::panic::catch_unwind(|| {
            SpectralFrame::new(vec![440.0, 880.0], vec![1.0], vec![0.0, 0.0]);
        });
        assert!(result.is_err());
    }

    #[test]
    fn morph_frames_alpha_zero_equals_a() {
        let a = make_frame(4, 220.0);
        let b = make_frame(4, 440.0);
        let m = morph_frames(&a, &b, 0.0);
        for i in 0..4 {
            assert!((m.frequencies[i] - a.frequencies[i]).abs() < 1e-9);
            assert!((m.amplitudes[i] - a.amplitudes[i]).abs() < 1e-9);
        }
    }

    #[test]
    fn morph_frames_alpha_one_equals_b() {
        let a = make_frame(4, 220.0);
        let b = make_frame(4, 440.0);
        let m = morph_frames(&a, &b, 1.0);
        for i in 0..4 {
            assert!((m.frequencies[i] - b.frequencies[i]).abs() < 1e-9);
            assert!((m.amplitudes[i] - b.amplitudes[i]).abs() < 1e-9);
        }
    }

    #[test]
    fn morph_frames_midpoint() {
        let a = make_frame(2, 100.0);
        let b = make_frame(2, 200.0);
        let m = morph_frames(&a, &b, 0.5);
        // Frequencies should be midway: [150, 300]
        assert!((m.frequencies[0] - 150.0).abs() < 1e-9);
        assert!((m.frequencies[1] - 300.0).abs() < 1e-9);
    }

    #[test]
    fn synthesize_frame_correct_length() {
        let frame = make_frame(3, 440.0);
        let samples = synthesize_frame(&frame, 1024, 44100);
        assert_eq!(samples.len(), 1024);
    }

    #[test]
    fn synthesize_frame_within_range() {
        let frame = make_frame(5, 440.0);
        let samples = synthesize_frame(&frame, 512, 44100);
        for &s in &samples {
            assert!(s >= -1.0 && s <= 1.0, "sample out of range: {}", s);
        }
    }

    #[test]
    fn morph_sequence_correct_total_length() {
        let morpher = SpectralMorpher::new();
        let source = make_frame(3, 220.0);
        let target = make_frame(3, 440.0);
        let output = morpher.morph_sequence(&source, &target, 4, 44100, 256);
        assert_eq!(output.len(), 4 * 256);
    }

    #[test]
    fn morph_sequence_zero_steps_empty() {
        let morpher = SpectralMorpher::new();
        let frame = make_frame(2, 440.0);
        let output = morpher.morph_sequence(&frame, &frame, 0, 44100, 256);
        assert!(output.is_empty());
    }

    #[test]
    fn cross_synthesis_uses_carrier_freqs() {
        let morpher = SpectralMorpher::new();
        let carrier = make_frame(4, 440.0);
        let modulator = make_frame(4, 220.0);
        let result = morpher.cross_synthesis(&carrier, &modulator);
        // Frequencies should come from carrier.
        for i in 0..4 {
            assert!((result.frequencies[i] - carrier.frequencies[i]).abs() < 1e-9);
            // Amplitudes should come from modulator.
            assert!((result.amplitudes[i] - modulator.amplitudes[i]).abs() < 1e-9);
        }
    }

    #[test]
    fn analyze_spectrum_returns_n_partials() {
        let samples: Vec<f64> = (0..1024)
            .map(|i| (2.0 * PI * 440.0 * i as f64 / 44100.0).sin())
            .collect();
        let frame = analyze_spectrum(&samples, 5, 440.0);
        assert_eq!(frame.n_partials(), 5);
    }

    #[test]
    fn analyze_spectrum_first_partial_is_fundamental() {
        let frame = analyze_spectrum(&vec![0.0; 512], 3, 440.0);
        assert!((frame.frequencies[0] - 440.0).abs() < 1e-9);
        assert!((frame.frequencies[1] - 880.0).abs() < 1e-9);
    }
}
