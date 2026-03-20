/// Resonator bank: 8 high-Q bandpass filters tuned to a major chord.
///
/// Noise excitation is run through all 8 filters and summed with stereo spread.
/// This produces a pitched, resonant texture driven by the attractor trajectory.
use crate::synth::BiquadFilter;

/// Default tuning: A major chord across two octaves.
const DEFAULT_FREQS: [f32; 8] = [220.0, 277.0, 330.0, 370.0, 440.0, 554.0, 660.0, 880.0];

pub struct ResonatorBank {
    filters: Vec<BiquadFilter>,
    /// Tuned frequencies for each filter (Hz).
    pub frequencies: [f32; 8],
    /// Filter resonance Q (default 12.0).
    pub q: f32,
    /// Noise excitation level scalar.
    pub excite_level: f32,
    sr: f32,
    /// xorshift64 state for noise generation.
    rng_state: u64,
}

impl ResonatorBank {
    /// Create a new `ResonatorBank` tuned to a major chord at the given sample rate.
    pub fn new(sr: f32) -> Self {
        let q = 12.0f32;
        let frequencies = DEFAULT_FREQS;
        let filters = frequencies
            .iter()
            .map(|&f| BiquadFilter::band_pass(f, q, sr))
            .collect();
        Self {
            filters,
            frequencies,
            q,
            excite_level: 1.0,
            sr,
            rng_state: 0xDEAD_BEEF_CAFE_BABE_u64,
        }
    }

    /// Retune all filters to the current `frequencies` and `q` values.
    pub fn update(&mut self) {
        for (filter, &freq) in self.filters.iter_mut().zip(self.frequencies.iter()) {
            filter.update_bp(freq, self.q, self.sr);
        }
    }

    /// xorshift64 — returns a noise sample in [-1, 1].
    fn next_noise(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        // Map top 32 bits to [-1, 1]
        (self.rng_state >> 32) as f32 / (u32::MAX as f32 / 2.0) - 1.0
    }

    /// Process one noise sample through all resonator filters.
    ///
    /// Returns a stereo (L, R) pair with alternate filters spread left and right.
    pub fn process(&mut self, noise_sample: f32) -> (f32, f32) {
        let mut l = 0.0f32;
        let mut r = 0.0f32;
        for (i, filter) in self.filters.iter_mut().enumerate() {
            let out = filter.process(noise_sample);
            if i % 2 == 0 {
                l += out;
            } else {
                r += out;
            }
        }
        // Normalize by number of filters per channel (4 each)
        (l * 0.25, r * 0.25)
    }

    /// Generate internal noise and process it, scaling by `excite_level`.
    pub fn next_sample(&mut self) -> (f32, f32) {
        let noise = self.next_noise() * self.excite_level;
        self.process(noise)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_resonator_output_finite() {
        let mut bank = ResonatorBank::new(SR);
        for _ in 0..2000 {
            let (l, r) = bank.next_sample();
            assert!(l.is_finite(), "Resonator L output non-finite");
            assert!(r.is_finite(), "Resonator R output non-finite");
        }
    }

    #[test]
    fn test_resonator_silent_with_zero_excitation() {
        let mut bank = ResonatorBank::new(SR);
        bank.excite_level = 0.0;
        // Warm up any stored state
        for _ in 0..1000 {
            bank.next_sample();
        }
        // With zero excitation, filters should drain to silence
        let (l, r) = bank.next_sample();
        assert!(
            l.abs() < 1e-4 && r.abs() < 1e-4,
            "Zero excitation should produce silence: l={}, r={}",
            l,
            r
        );
    }

    #[test]
    fn test_resonator_produces_output_with_excitation() {
        let mut bank = ResonatorBank::new(SR);
        bank.excite_level = 1.0;
        let mut max_abs = 0.0_f32;
        for _ in 0..2000 {
            let (l, r) = bank.next_sample();
            max_abs = max_abs.max(l.abs()).max(r.abs());
        }
        assert!(max_abs > 0.0, "Non-zero excitation should produce output");
    }
}
