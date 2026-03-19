use super::{quantize_to_scale, AudioParams, Scale, SonifMode, Sonification};
use crate::config::SonificationConfig;

/// Granular mode: trajectory speed → grain density; position → grain pitch.
pub struct GranularMapping {
    min_state: Vec<f64>,
    max_state: Vec<f64>,
}

impl GranularMapping {
    /// Creates a new `GranularMapping` with an empty min/max normalization window.
    ///
    /// The window is populated lazily on the first call to [`Sonification::map`].
    pub fn new() -> Self {
        Self {
            min_state: Vec::new(),
            max_state: Vec::new(),
        }
    }
}

impl Sonification for GranularMapping {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        // Initialize or expand tracking
        if self.min_state.len() != state.len() {
            self.min_state = state.to_vec();
            self.max_state = state.to_vec();
        }
        for (i, &v) in state.iter().enumerate() {
            if v < self.min_state[i] {
                self.min_state[i] = v;
            }
            if v > self.max_state[i] {
                self.max_state[i] = v;
            }
        }

        let scale: Scale = config.scale.clone().into();
        let base = config.base_frequency as f32;
        let oct = config.octave_range as f32;

        // Normalize first dimension to get base pitch
        let t = if state.is_empty() {
            0.5
        } else {
            let range = (self.max_state[0] - self.min_state[0]).abs().max(1e-9);
            ((state[0] - self.min_state[0]) / range) as f32
        };

        // Grain density: proportional to speed, clamped to 5-200 grains/sec
        let grain_rate = (speed.abs() as f32 * 2.0).clamp(5.0, 200.0);

        // Frequency spread from second dimension
        let spread = if state.len() > 1 {
            let range = (self.max_state[1] - self.min_state[1]).abs().max(1e-9);
            ((state[1] - self.min_state[1]) / range) as f32
        } else {
            0.5
        };

        let chaos_level = (speed.abs() as f32 / 200.0).clamp(0.0, 1.0);

        let mut p = AudioParams {
            mode: SonifMode::Granular,
            grain_spawn_rate: grain_rate,
            grain_base_freq: quantize_to_scale(t, base, oct, scale),
            grain_freq_spread: spread * 2.0,
            gain: 0.4,
            filter_cutoff: 4000.0,
            filter_q: 0.4 + chaos_level * 2.5,
            ..Default::default()
        };

        // Higher-dimension voice distribution: each state dimension drives its
        // own voice frequency, enabling systems with many dimensions (Lorenz96,
        // Kuramoto) to use all available state variables musically.
        for i in 0..4.min(state.len()) {
            let norm_i = {
                let range = (self.max_state[i] - self.min_state[i]).abs().max(1e-9);
                ((state[i] - self.min_state[i]) / range) as f32
            };
            p.freqs[i] = quantize_to_scale(norm_i, base, oct, scale);
            p.amps[i] = 0.5 + 0.5 * norm_i;
            p.pans[i] = norm_i * 2.0 - 1.0;
        }

        p.chaos_level = chaos_level;
        p
    }
}
