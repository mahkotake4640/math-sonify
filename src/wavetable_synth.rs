//! Wavetable synthesis with waveform morphing.

/// Waveform types for wavetable generation.
#[derive(Debug, Clone)]
pub enum Waveform {
    Sine,
    Square,
    Sawtooth,
    Triangle,
    /// LCG seed for noise generation.
    Noise(u64),
}

/// Size of each wavetable (one period).
pub const WAVETABLE_SIZE: usize = 2048;

/// Build a single-period wavetable for the given waveform.
pub fn build_wavetable(wave: &Waveform) -> Vec<f32> {
    let n = WAVETABLE_SIZE;
    match wave {
        Waveform::Sine => (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * i as f64 / n as f64).sin() as f32)
            .collect(),
        Waveform::Square => (0..n)
            .map(|i| {
                let s = (2.0 * std::f64::consts::PI * i as f64 / n as f64).sin();
                if s >= 0.0 { 1.0f32 } else { -1.0f32 }
            })
            .collect(),
        Waveform::Sawtooth => (0..n)
            .map(|i| (2.0 * (i as f64 / n as f64) - 1.0) as f32)
            .collect(),
        Waveform::Triangle => (0..n)
            .map(|i| {
                let t = i as f64 / n as f64;
                (2.0 * (2.0 * t - 1.0).abs() - 1.0) as f32
            })
            .collect(),
        Waveform::Noise(seed) => {
            let mut state = *seed;
            (0..n)
                .map(|_| {
                    // LCG parameters
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    let norm = (state >> 33) as f64 / u32::MAX as f64;
                    (norm * 2.0 - 1.0) as f32
                })
                .collect()
        }
    }
}

/// Single-waveform wavetable oscillator.
pub struct WavetableOscillator {
    pub table: Vec<f32>,
    pub phase: f64,
    pub freq: f64,
    pub sample_rate: f64,
    pub amplitude: f32,
}

impl WavetableOscillator {
    pub fn new(wave: Waveform, freq: f64, sample_rate: f64) -> Self {
        Self {
            table: build_wavetable(&wave),
            phase: 0.0,
            freq,
            sample_rate,
            amplitude: 1.0,
        }
    }

    /// Produce the next sample using linear interpolation.
    pub fn next_sample(&mut self) -> f32 {
        let n = self.table.len() as f64;
        let idx = self.phase * n;
        let i0 = idx.floor() as usize % self.table.len();
        let i1 = (i0 + 1) % self.table.len();
        let frac = (idx - idx.floor()) as f32;
        let sample = self.table[i0] * (1.0 - frac) + self.table[i1] * frac;

        self.phase += self.freq / self.sample_rate;
        self.phase -= self.phase.floor();

        sample * self.amplitude
    }

    /// Render num_samples samples.
    pub fn render(&mut self, num_samples: usize) -> Vec<f32> {
        (0..num_samples).map(|_| self.next_sample()).collect()
    }
}

/// Morphing oscillator between two wavetables.
pub struct WaveformMorpher {
    pub table_a: Vec<f32>,
    pub table_b: Vec<f32>,
    pub phase: f64,
    pub freq: f64,
    pub sample_rate: f64,
}

impl WaveformMorpher {
    pub fn new(wave_a: Waveform, wave_b: Waveform, freq: f64, sample_rate: f64) -> Self {
        Self {
            table_a: build_wavetable(&wave_a),
            table_b: build_wavetable(&wave_b),
            phase: 0.0,
            freq,
            sample_rate,
        }
    }

    /// Get a sample blended between table_a and table_b by morph ∈ [0,1].
    pub fn next_sample(&mut self, morph: f32) -> f32 {
        let morph = morph.clamp(0.0, 1.0);
        let n = self.table_a.len() as f64;
        let idx = self.phase * n;
        let i0 = idx.floor() as usize % self.table_a.len();
        let i1 = (i0 + 1) % self.table_a.len();
        let frac = (idx - idx.floor()) as f32;

        let a = self.table_a[i0] * (1.0 - frac) + self.table_a[i1] * frac;
        let b = self.table_b[i0] * (1.0 - frac) + self.table_b[i1] * frac;

        self.phase += self.freq / self.sample_rate;
        self.phase -= self.phase.floor();

        (1.0 - morph) * a + morph * b
    }

    /// Render with per-sample morph curve (wraps around if shorter than num_samples).
    pub fn render(&mut self, num_samples: usize, morph_curve: &[f32]) -> Vec<f32> {
        if morph_curve.is_empty() {
            return (0..num_samples).map(|_| self.next_sample(0.0)).collect();
        }
        (0..num_samples)
            .map(|i| {
                let m = morph_curve[i % morph_curve.len()];
                self.next_sample(m)
            })
            .collect()
    }
}

/// Multi-voice oscillator with per-voice detuning.
pub struct MultiOscillator {
    pub oscillators: Vec<WavetableOscillator>,
    pub detune_semitones: Vec<f32>,
}

impl MultiOscillator {
    /// Create num_voices oscillators detuned symmetrically around freq.
    pub fn new(
        wave: Waveform,
        freq: f64,
        num_voices: usize,
        detune_semitones: f32,
        sample_rate: f64,
    ) -> Self {
        let mut oscillators = Vec::with_capacity(num_voices);
        let mut detune_vec = Vec::with_capacity(num_voices);

        for i in 0..num_voices {
            let offset = if num_voices > 1 {
                i as f32 * detune_semitones / (num_voices as f32 - 1.0) - detune_semitones / 2.0
            } else {
                0.0
            };
            let detuned_freq = freq * 2.0_f64.powf(offset as f64 / 12.0);
            oscillators.push(WavetableOscillator::new(wave.clone(), detuned_freq, sample_rate));
            detune_vec.push(offset);
        }

        Self {
            oscillators,
            detune_semitones: detune_vec,
        }
    }

    /// Render, sum all voices and normalize by num_voices.
    pub fn render(&mut self, num_samples: usize) -> Vec<f32> {
        let nv = self.oscillators.len().max(1) as f32;
        let mut output = vec![0.0f32; num_samples];
        for osc in &mut self.oscillators {
            for (o, s) in output.iter_mut().zip(osc.render(num_samples).iter()) {
                *o += s / nv;
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wavetable_size() {
        let table = build_wavetable(&Waveform::Sine);
        assert_eq!(table.len(), WAVETABLE_SIZE);
    }

    #[test]
    fn test_sine_at_half_period_approx_zero() {
        let table = build_wavetable(&Waveform::Sine);
        // At exactly half period the sine should be ~0
        let half = table[WAVETABLE_SIZE / 2];
        assert!(half.abs() < 0.01, "half period sine should be ~0, got {half}");
    }

    #[test]
    fn test_square_is_plus_minus_one_only() {
        let table = build_wavetable(&Waveform::Square);
        for s in &table {
            assert!((s.abs() - 1.0).abs() < 1e-6, "square value not ±1: {s}");
        }
    }

    #[test]
    fn test_render_returns_correct_length() {
        let mut osc = WavetableOscillator::new(Waveform::Sine, 440.0, 44100.0);
        let samples = osc.render(1024);
        assert_eq!(samples.len(), 1024);
    }

    #[test]
    fn test_morph_at_zero_equals_table_a() {
        // With morph=0 output should come entirely from table_a
        let mut morpher = WaveformMorpher::new(Waveform::Sine, Waveform::Square, 440.0, 44100.0);
        let curve = vec![0.0f32; 64];
        let out = morpher.render(64, &curve);

        // Reset and compare with pure sine oscillator
        let mut sine_osc = WavetableOscillator::new(Waveform::Sine, 440.0, 44100.0);
        let sine_out = sine_osc.render(64);

        for (a, b) in out.iter().zip(sine_out.iter()) {
            assert!((a - b).abs() < 1e-5, "morph=0 output differs from table_a");
        }
    }

    #[test]
    fn test_morph_at_one_equals_table_b() {
        let mut morpher = WaveformMorpher::new(Waveform::Sine, Waveform::Square, 440.0, 44100.0);
        let curve = vec![1.0f32; 64];
        let out = morpher.render(64, &curve);

        let mut sq_osc = WavetableOscillator::new(Waveform::Square, 440.0, 44100.0);
        let sq_out = sq_osc.render(64);

        for (a, b) in out.iter().zip(sq_out.iter()) {
            assert!((a - b).abs() < 1e-5, "morph=1 output differs from table_b");
        }
    }
}
