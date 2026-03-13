use super::{AudioParams, Sonification, SonifMode};
use crate::config::SonificationConfig;

const NUM_PARTIALS: usize = 32;

/// Spectral mode: state vector → spectral envelope via additive synthesis.
/// The state defines the shape; we normalize and map dimensions cyclically to partials.
pub struct SpectralMapping {
    smoothed: [f32; NUM_PARTIALS],
    alpha: f32,
}

impl SpectralMapping {
    pub fn new() -> Self {
        Self { smoothed: [0.0; NUM_PARTIALS], alpha: 0.05 }
    }
}

impl Sonification for SpectralMapping {
    fn map(&mut self, state: &[f64], _speed: f64, config: &SonificationConfig) -> AudioParams {
        if state.is_empty() { return AudioParams { mode: SonifMode::Spectral, ..Default::default() }; }

        // Normalize state magnitudes to [0,1]
        let max_abs = state.iter().map(|v| v.abs()).fold(0.0f64, f64::max).max(1e-9);

        let mut raw = [0.0f32; NUM_PARTIALS];
        for (k, slot) in raw.iter_mut().enumerate() {
            let i = k % state.len();
            *slot = (state[i].abs() / max_abs) as f32;
        }

        // Apply a spectral roll-off: higher partials naturally quieter
        for (k, slot) in raw.iter_mut().enumerate() {
            *slot *= 1.0 / (1.0 + k as f32 * 0.15);
        }

        // Smooth transitions to prevent clicks
        for (k, s) in self.smoothed.iter_mut().enumerate() {
            *s = *s + self.alpha * (raw[k] - *s);
        }

        let base = config.base_frequency as f32;
        let mut p = AudioParams {
            mode: SonifMode::Spectral,
            partials: self.smoothed,
            partials_base_freq: base,
            gain: 0.15,
            filter_cutoff: 8000.0,
            filter_q: 0.7,
            ..Default::default()
        };
        p.chaos_level = 0.3;
        p
    }
}
