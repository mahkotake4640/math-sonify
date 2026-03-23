//! Custom waveform generator: oscillators, mixers, and additive synthesis.
//!
//! All phase values are in `[0, 1)` and all output amplitudes are in `[-1, 1]`
//! before gain scaling.

use std::f64::consts::PI;

// ── WaveformType ──────────────────────────────────────────────────────────────

/// Selects the waveform shape produced by an [`Oscillator`].
#[derive(Debug, Clone)]
pub enum WaveformType {
    /// Sinusoidal wave: `sin(2π·phase)`.
    Sine,
    /// Bipolar square wave (±1).
    Square,
    /// Rising sawtooth: `2·phase - 1`.
    Sawtooth,
    /// Symmetric triangle wave.
    Triangle,
    /// Pulse wave with configurable duty cycle.
    Pulse {
        /// Fraction of cycle spent high (0..1).
        duty_cycle: f64,
    },
    /// White noise via a deterministic LCG seeded from phase.
    WhiteNoise,
    /// Pink noise via Voss-McCartney 6-octave summed white noise.
    PinkNoise,
}

/// Compute a single sample for the given waveform type and phase.
///
/// `phase` must be in `[0, 1)`. Output is in `[-1, 1]`.
pub fn sample(waveform: &WaveformType, phase: f64) -> f64 {
    match waveform {
        WaveformType::Sine => (2.0 * PI * phase).sin(),
        WaveformType::Square => {
            if phase < 0.5 { 1.0 } else { -1.0 }
        }
        WaveformType::Sawtooth => 2.0 * phase - 1.0,
        WaveformType::Triangle => {
            if phase < 0.5 {
                4.0 * phase - 1.0
            } else {
                3.0 - 4.0 * phase
            }
        }
        WaveformType::Pulse { duty_cycle } => {
            if phase < *duty_cycle { 1.0 } else { -1.0 }
        }
        WaveformType::WhiteNoise => {
            // Deterministic LCG seeded from quantised phase.
            let seed = (phase * 1_000_000.0) as u64;
            let v = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            // Map to [-1, 1].
            (v as f64 / u64::MAX as f64) * 2.0 - 1.0
        }
        WaveformType::PinkNoise => {
            // Voss-McCartney: sum 6 white-noise sources each updated at half
            // the previous rate (simulated from phase).
            let mut out = 0.0f64;
            for octave in 0..6u64 {
                let period = 1u64 << octave; // 1, 2, 4, 8, 16, 32 samples per update
                let slot = ((phase * 1024.0) as u64) / period;
                let seed = slot
                    .wrapping_mul(0xbf58476d1ce4e5b9u64.wrapping_add(octave * 0x123456789))
                    .wrapping_add(1442695040888963407);
                out += (seed as f64 / u64::MAX as f64) * 2.0 - 1.0;
            }
            (out / 6.0).clamp(-1.0, 1.0)
        }
    }
}

// ── Oscillator ────────────────────────────────────────────────────────────────

/// Single oscillator with a configurable waveform.
pub struct Oscillator {
    /// Waveform shape.
    pub waveform: WaveformType,
    /// Frequency in Hz.
    pub frequency_hz: f64,
    /// Peak amplitude scalar (multiplied into each sample).
    pub amplitude: f64,
    /// Initial phase offset in `[0, 1)`.
    pub phase_offset: f64,
    /// Audio sample rate in Hz.
    pub sample_rate: f64,
    /// Internal accumulated phase in `[0, 1)`.
    phase: f64,
}

impl Oscillator {
    /// Create a new oscillator.
    pub fn new(
        waveform: WaveformType,
        frequency_hz: f64,
        amplitude: f64,
        phase_offset: f64,
        sample_rate: f64,
    ) -> Self {
        Oscillator {
            waveform,
            frequency_hz,
            amplitude,
            phase_offset,
            sample_rate,
            phase: phase_offset,
        }
    }

    /// Generate `duration_secs` worth of samples.
    pub fn generate(&mut self, duration_secs: f64) -> Vec<f64> {
        let n_samples = (duration_secs * self.sample_rate).round() as usize;
        let phase_inc = self.frequency_hz / self.sample_rate;
        let mut out = Vec::with_capacity(n_samples);
        for _ in 0..n_samples {
            out.push(self.amplitude * sample(&self.waveform, self.phase));
            self.phase = (self.phase + phase_inc).fract();
        }
        out
    }

    /// Advance the internal phase accumulator by `n_samples` without generating audio.
    pub fn advance_phase(&mut self, n_samples: usize) {
        let phase_inc = self.frequency_hz / self.sample_rate;
        self.phase = (self.phase + phase_inc * n_samples as f64).fract();
    }

    /// Reset phase to the configured offset.
    pub fn reset(&mut self) {
        self.phase = self.phase_offset;
    }
}

// ── WaveformMixer ─────────────────────────────────────────────────────────────

/// Mixes multiple oscillators with individual gain values.
pub struct WaveformMixer {
    /// `(oscillator, gain)` pairs.
    pub oscillators: Vec<(Oscillator, f64)>,
}

impl WaveformMixer {
    /// Create an empty mixer.
    pub fn new() -> Self {
        WaveformMixer { oscillators: Vec::new() }
    }

    /// Add an oscillator with the given gain.
    pub fn add_oscillator(&mut self, osc: Oscillator, gain: f64) {
        self.oscillators.push((osc, gain));
    }

    /// Sum all oscillator outputs scaled by their gains, then normalize to `[-1, 1]`.
    pub fn mix(&mut self, duration_secs: f64) -> Vec<f64> {
        if self.oscillators.is_empty() {
            return Vec::new();
        }

        // All oscillators must agree on sample rate; use first.
        let n_samples = (duration_secs * self.oscillators[0].0.sample_rate).round() as usize;
        let mut out = vec![0.0f64; n_samples];

        for (osc, gain) in &mut self.oscillators {
            let buf = osc.generate(duration_secs);
            for (o, s) in out.iter_mut().zip(buf.iter()) {
                *o += s * *gain;
            }
        }

        // Normalize.
        let peak = out.iter().cloned().map(f64::abs).fold(0.0f64, f64::max);
        if peak > 1e-12 {
            for s in &mut out {
                *s /= peak;
            }
        }
        out
    }
}

impl Default for WaveformMixer {
    fn default() -> Self {
        Self::new()
    }
}

// ── AdditiveOscillator ────────────────────────────────────────────────────────

/// Additive synthesizer: sum of harmonically related sinusoids.
pub struct AdditiveOscillator {
    /// Fundamental frequency in Hz.
    pub fundamental_hz: f64,
    /// `(harmonic_number, relative_amplitude)` pairs.
    pub harmonics: Vec<(u32, f64)>,
    /// Audio sample rate in Hz.
    pub sample_rate: f64,
}

impl AdditiveOscillator {
    /// Create an additive oscillator from explicit harmonic pairs.
    pub fn new(fundamental_hz: f64, harmonics: Vec<(u32, f64)>, sample_rate: f64) -> Self {
        AdditiveOscillator { fundamental_hz, harmonics, sample_rate }
    }

    /// Generate `duration_secs` of audio by summing all harmonics.
    pub fn generate(&self, duration_secs: f64) -> Vec<f64> {
        let n_samples = (duration_secs * self.sample_rate).round() as usize;
        let mut out = vec![0.0f64; n_samples];
        for (h_num, h_amp) in &self.harmonics {
            let freq = self.fundamental_hz * (*h_num as f64);
            let phase_inc = freq / self.sample_rate;
            let mut phase = 0.0f64;
            for s in &mut out {
                *s += h_amp * (2.0 * PI * phase).sin();
                phase = (phase + phase_inc).fract();
            }
        }
        // Normalize by sum of amplitudes.
        let amp_sum: f64 = self.harmonics.iter().map(|(_, a)| a.abs()).sum();
        if amp_sum > 1e-12 {
            for s in &mut out {
                *s /= amp_sum;
            }
        }
        out
    }

    /// Harmonic series for a sawtooth wave: `[(1, 1.0), (2, 0.5), (3, 1/3), …]`.
    pub fn sawtooth_harmonics(n: usize) -> Vec<(u32, f64)> {
        (1..=n).map(|k| (k as u32, 1.0 / k as f64)).collect()
    }

    /// Harmonic series for a square wave: odd harmonics only.
    pub fn square_harmonics(n: usize) -> Vec<(u32, f64)> {
        (0..n)
            .map(|k| {
                let h = (2 * k + 1) as u32;
                (h, 1.0 / h as f64)
            })
            .collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 44100.0;

    #[test]
    fn sine_peak_equals_amplitude() {
        let mut osc = Oscillator::new(WaveformType::Sine, 440.0, 0.8, 0.0, SR);
        let buf = osc.generate(0.1);
        let peak = buf.iter().cloned().map(f64::abs).fold(0.0f64, f64::max);
        assert!((peak - 0.8).abs() < 0.01, "peak={peak}");
    }

    #[test]
    fn square_has_only_two_values() {
        let mut osc = Oscillator::new(WaveformType::Square, 440.0, 1.0, 0.0, SR);
        let buf = osc.generate(0.1);
        let mut vals: Vec<f64> = buf.clone();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        vals.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert_eq!(vals.len(), 2, "square wave should have exactly 2 distinct values, got {:?}", vals);
    }

    #[test]
    fn sawtooth_is_linear_within_cycle() {
        // Check that sawtooth increments monotonically within a single period.
        let freq = 100.0;
        let period_samples = (SR / freq).round() as usize;
        let mut osc = Oscillator::new(WaveformType::Sawtooth, freq, 1.0, 0.0, SR);
        let buf = osc.generate(1.0 / freq * 0.9); // 90% of one period
        // The values should be monotonically increasing.
        let mut increasing = true;
        for pair in buf.windows(2) {
            if pair[1] < pair[0] - 1e-9 {
                increasing = false;
                break;
            }
        }
        assert!(increasing, "sawtooth should be monotonically increasing within a period; first {} samples", period_samples);
    }

    #[test]
    fn mix_normalizes_within_bounds() {
        let mut mixer = WaveformMixer::new();
        mixer.add_oscillator(Oscillator::new(WaveformType::Sine, 440.0, 1.0, 0.0, SR), 1.0);
        mixer.add_oscillator(Oscillator::new(WaveformType::Square, 880.0, 1.0, 0.0, SR), 1.0);
        let buf = mixer.mix(0.1);
        let peak = buf.iter().cloned().map(f64::abs).fold(0.0f64, f64::max);
        assert!(peak <= 1.0 + 1e-9, "mix peak out of bounds: {peak}");
    }

    #[test]
    fn additive_sawtooth_harmonics_count() {
        let h = AdditiveOscillator::sawtooth_harmonics(8);
        assert_eq!(h.len(), 8);
        assert_eq!(h[0], (1, 1.0));
        assert!((h[1].1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn additive_square_harmonics_are_odd() {
        let h = AdditiveOscillator::square_harmonics(4);
        let nums: Vec<u32> = h.iter().map(|(n, _)| *n).collect();
        assert_eq!(nums, vec![1, 3, 5, 7]);
    }
}
