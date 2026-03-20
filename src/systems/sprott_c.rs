use super::{rk4, DynamicalSystem};

/// Sprott C system: dx/dt = y·z, dy/dt = x − y, dz/dt = 1 − x²
///
/// One of Sprott's algebraically simplest three-dimensional chaotic flows.
/// No free parameters; the strange attractor is an invariant set of the fixed
/// equations. The attractor has a characteristic butterfly shape distinct from
/// Sprott B and produces richer trajectory dynamics (larger Lyapunov exponent).
pub struct SprottC {
    state: Vec<f64>,
    speed: f64,
}

impl SprottC {
    /// Create a Sprott C attractor with initial state (0.1, 0.0, 0.0).
    pub fn new() -> Self {
        Self {
            state: vec![0.1, 0.0, 0.0],
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64]) -> Vec<f64> {
        vec![s[1] * s[2], s[0] - s[1], 1.0 - s[0] * s[0]]
    }
}

impl Default for SprottC {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for SprottC {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "sprott_c"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state)
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
        rk4(&mut self.state, dt, Self::deriv);
        let ds: f64 = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        self.speed = ds / dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_sprott_c_initial_state() {
        let sys = SprottC::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "sprott_c");
        let s = sys.state();
        assert!((s[0] - 0.1).abs() < 1e-15);
        assert_eq!(s[1], 0.0);
        assert_eq!(s[2], 0.0);
    }

    #[test]
    fn test_sprott_c_step_changes_state() {
        let mut sys = SprottC::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_sprott_c_state_stays_finite() {
        let mut sys = SprottC::new();
        for _ in 0..1000 {
            sys.step(0.01);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_sprott_c_deterministic() {
        let mut s1 = SprottC::new();
        let mut s2 = SprottC::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_sprott_c_set_state() {
        let mut sys = SprottC::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_sprott_c_set_state_ignores_nan() {
        let mut sys = SprottC::new();
        sys.set_state(&[f64::NAN, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 0.1).abs() < 1e-15, "NaN should not change state[0]");
        assert!((s[1] - 2.0).abs() < 1e-15);
    }
}
