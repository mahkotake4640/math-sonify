use super::DynamicalSystem;
use std::f64::consts::TAU;

/// Chirikov–Taylor standard map — an area-preserving 2D discrete map.
///
/// Iteration rule (all angles in [0, 2π)):
/// ```text
/// p_{n+1} = (p + k · sin θ) mod 2π
/// θ_{n+1} = (θ + p_{n+1}) mod 2π
/// ```
/// For k = 0 the motion is integrable (circles in phase space).
/// For k ≈ 0.97 the last KAM torus breaks, yielding global chaos.
/// For large k the phase space is almost entirely chaotic.
///
/// The state is (θ, p, k) so the sonification pipeline gets both
/// the angle and the momentum as independent signal dimensions.
pub struct StandardMap {
    state: Vec<f64>,
    pub k: f64,
    speed: f64,
}

impl StandardMap {
    /// Create a standard map with the given stochasticity parameter k.
    /// Default k = 1.5 gives strong but not complete chaos.
    pub fn new(k: f64) -> Self {
        Self {
            state: vec![0.5, 0.5, k],
            k,
            speed: 0.0,
        }
    }
}

impl DynamicalSystem for StandardMap {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "standard_map"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, _state: &[f64]) -> Vec<f64> {
        vec![0.0; 3]
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }

    fn step(&mut self, _dt: f64) {
        let theta = self.state[0];
        let p = self.state[1];

        let new_p = (p + self.k * theta.sin()).rem_euclid(TAU);
        let new_theta = (theta + new_p).rem_euclid(TAU);

        let delta = ((new_theta - theta).powi(2) + (new_p - p).powi(2)).sqrt();
        self.speed = delta;

        self.state[0] = new_theta;
        self.state[1] = new_p;
        self.state[2] = self.k;
    }
}
