use super::{AudioParams, Sonification, SonifMode};
use crate::config::SonificationConfig;

/// Orbital resonance: compute instantaneous angular velocity in projected 2D plane.
/// Use it as fundamental; harmonics derived from local divergence rate.
pub struct OrbitalResonance {
    prev_state: Vec<f64>,
    prev_angle: Option<f64>,
    lyap_estimate: f64,
}

impl OrbitalResonance {
    pub fn new() -> Self {
        Self { prev_state: Vec::new(), prev_angle: None, lyap_estimate: 0.0 }
    }
}

impl Sonification for OrbitalResonance {
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams {
        let mut params = AudioParams {
            mode: SonifMode::Orbital,
            gain: 0.2,
            filter_cutoff: 1500.0,
            filter_q: 1.2,
            ..Default::default()
        };

        // Project onto first two dimensions
        let (x, y) = (state[0], state[1]);
        let angle = y.atan2(x);

        let angular_vel = if let Some(prev_a) = self.prev_angle {
            let da = angle - prev_a;
            // Unwrap angle difference
            let da_unwrapped = if da > std::f64::consts::PI { da - std::f64::consts::TAU }
                               else if da < -std::f64::consts::PI { da + std::f64::consts::TAU }
                               else { da };
            da_unwrapped.abs() * 60.0 // scale to reasonable Hz range
        } else { 1.0 };
        self.prev_angle = Some(angle);

        // Update Lyapunov estimate (finite-time, single perturbation direction)
        if !self.prev_state.is_empty() {
            let divergence: f64 = state.iter().zip(self.prev_state.iter())
                .map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
            let log_div = divergence.ln().clamp(-5.0, 5.0);
            self.lyap_estimate = self.lyap_estimate * 0.99 + log_div * 0.01;
        }
        self.prev_state = state.to_vec();

        let base = config.base_frequency as f32;
        // Clamp angular velocity to a musically useful range (1/16x to 4x of base)
        let fundamental = (angular_vel.abs() as f32 * base * 0.05).clamp(base * 0.0625, base * 4.0);

        // Inharmonicity driven by Lyapunov estimate: 0 = harmonic, 1 = stretched
        let stretch = (self.lyap_estimate.tanh() * 0.5 + 0.5) as f32;

        for i in 0..4 {
            let harmonic = (i + 1) as f32;
            // Inharmonic stretch: f_n = f1 * n^(1 + stretch*0.3)
            params.freqs[i] = fundamental * harmonic.powf(1.0 + stretch * 0.3);
            params.amps[i] = 0.8 / harmonic; // 1/n amplitude falloff
            params.pans[i] = (i as f32 / 3.0) * 2.0 - 1.0;
        }

        // Filter tracks chaos: more chaos → brighter
        params.filter_cutoff = 500.0 + 3500.0 * stretch;
        params.filter_q = 0.5 + 1.5 * (1.0 - stretch);
        params.gain = (0.15 + 0.1 * speed.tanh() as f32).min(0.4);
        params.chaos_level = stretch;
        params
    }
}
