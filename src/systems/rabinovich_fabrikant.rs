use super::{rk4, DynamicalSystem};

/// Rabinovich–Fabrikant attractor — a dissipative system derived from plasma
/// physics modelling stochastic oscillations in certain non-equilibrium media.
///
/// Equations:
/// ```text
/// dx/dt = y·(z − 1 + x²) + γ·x
/// dy/dt = x·(3z + 1 − x²) + γ·y
/// dz/dt = −2z·(α + x·y)
/// ```
///
/// With α=0.14, γ=0.1 the system exhibits a complex strange attractor with a
/// distinctive multi-lobe structure.  Reducing γ toward 0 makes it more chaotic.
pub struct RabinovichFabrikant {
    state: Vec<f64>,
    /// Damping-like parameter. Default 0.14.
    pub alpha: f64,
    /// Excitation parameter. Default 0.1.
    pub gamma: f64,
    speed: f64,
}

impl RabinovichFabrikant {
    /// Create a Rabinovich-Fabrikant attractor with default parameters (α=0.14, γ=0.1).
    pub fn new() -> Self {
        Self {
            state: vec![-1.0, 0.0, 0.5],
            alpha: 0.14,
            gamma: 0.1,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], alpha: f64, gamma: f64) -> Vec<f64> {
        vec![
            s[1] * (s[2] - 1.0 + s[0] * s[0]) + gamma * s[0],
            s[0] * (3.0 * s[2] + 1.0 - s[0] * s[0]) + gamma * s[1],
            -2.0 * s[2] * (alpha + s[0] * s[1]),
        ]
    }
}

impl Default for RabinovichFabrikant {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for RabinovichFabrikant {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "rabinovich_fabrikant"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.alpha, self.gamma)
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }

    fn step(&mut self, dt: f64) {
        let prev = self.state.clone();
        let (alpha, gamma) = (self.alpha, self.gamma);
        rk4(&mut self.state, dt, |s| Self::deriv(s, alpha, gamma));
        self.speed = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
            / dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_rf_initial_state() {
        let sys = RabinovichFabrikant::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "rabinovich_fabrikant");
        assert!(sys.state().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_rf_step_changes_state() {
        let mut sys = RabinovichFabrikant::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.001);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_rf_state_stays_finite() {
        let mut sys = RabinovichFabrikant::new();
        for _ in 0..5000 {
            sys.step(0.001);
        }
        for v in sys.state() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_rf_deterministic() {
        let mut s1 = RabinovichFabrikant::new();
        let mut s2 = RabinovichFabrikant::new();
        for _ in 0..200 {
            s1.step(0.001);
            s2.step(0.001);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_rf_set_state() {
        let mut sys = RabinovichFabrikant::new();
        sys.set_state(&[1.0, 2.0, 0.3]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 0.3).abs() < 1e-15);
    }

    #[test]
    fn test_rf_deriv_at_known_point() {
        // At (1, 1, 1) with α=0.14, γ=0.1:
        // dx = 1*(1 - 1 + 1) + 0.1*1 = 1.1
        // dy = 1*(3 + 1 - 1) + 0.1*1 = 3.1
        // dz = -2*1*(0.14 + 1*1) = -2*(1.14) = -2.28
        let sys = RabinovichFabrikant::new();
        let d = sys.deriv_at(&[1.0, 1.0, 1.0]);
        assert!((d[0] - 1.1).abs() < 1e-12, "d[0]={}", d[0]);
        assert!((d[1] - 3.1).abs() < 1e-12, "d[1]={}", d[1]);
        assert!((d[2] - (-2.28)).abs() < 1e-12, "d[2]={}", d[2]);
    }

    #[test]
    fn test_rf_speed_positive_after_step() {
        let mut sys = RabinovichFabrikant::new();
        sys.step(0.001);
        assert!(sys.speed() > 0.0, "Speed should be positive after a step");
    }
}
