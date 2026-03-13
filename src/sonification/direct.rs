use super::{AudioParams, Sonification, SonifMode, Scale, quantize_to_scale};
use crate::config::SonificationConfig;

/// Direct frequency mapping: each state variable → a voice frequency.
/// Variables are normalized to [0,1] via a rolling min/max window, then
/// quantized to the selected scale.
pub struct DirectMapping {
    min: Vec<f64>,
    max: Vec<f64>,
    alpha: f64, // exponential moving average for min/max tracking
}

impl DirectMapping {
    pub fn new() -> Self {
        Self { min: Vec::new(), max: Vec::new(), alpha: 0.001 }
    }

    fn normalize(&mut self, state: &[f64]) -> Vec<f32> {
        if self.min.len() != state.len() {
            self.min = state.to_vec();
            self.max = state.to_vec();
        }
        state.iter().enumerate().map(|(i, &v)| {
            // Soft min/max tracking
            if v < self.min[i] { self.min[i] = v; }
            else { self.min[i] += self.alpha * (v - self.min[i]); }
            if v > self.max[i] { self.max[i] = v; }
            else { self.max[i] += self.alpha * (v - self.max[i]); }
            let range = (self.max[i] - self.min[i]).abs().max(1e-9);
            ((v - self.min[i]) / range) as f32
        }).collect()
    }
}

impl Sonification for DirectMapping {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        let norm = self.normalize(state);
        let scale: Scale = config.scale.clone().into();
        let base = config.base_frequency as f32;
        let oct = config.octave_range as f32;

        let mut params = AudioParams {
            mode: SonifMode::Direct,
            gain: 0.25,
            filter_cutoff: 2000.0,
            filter_q: 0.7,
            ..Default::default()
        };

        // Up to 4 voices from the first 4 state variables
        for i in 0..4.min(norm.len()) {
            params.freqs[i] = quantize_to_scale(norm[i], base, oct, scale);
            params.amps[i] = if i < norm.len() { 0.5 + 0.5 * norm[i] } else { 0.0 };
            params.pans[i] = norm[i] * 2.0 - 1.0;
        }
        // Use last dimension to modulate filter cutoff
        if let Some(&last) = norm.last() {
            params.filter_cutoff = 300.0 + 3700.0 * last;
        }
        params.chaos_level = (speed.abs() as f32 / 100.0).clamp(0.0, 1.0);
        params
    }
}
