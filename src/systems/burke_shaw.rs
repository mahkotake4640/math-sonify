use super::{rk4, DynamicalSystem};

/// Burke-Shaw attractor — a two-parameter three-dimensional chaotic system.
///
/// Equations:
/// ```text
/// dx/dt = −σ·(x + y)
/// dy/dt = −y − σ·x·z
/// dz/dt = σ·x·y + ρ
/// ```
///
/// With σ=10, ρ=4.272 the system exhibits robust chaos and a distinctive
/// two-scroll structure.  The Burke-Shaw system is notable for its very simple
/// algebraic form and clean double-lobe attractor geometry.
pub struct BurkeShaw {
    state: Vec<f64>,
    pub sigma: f64,
    pub rho: f64,
    speed: f64,
}

impl BurkeShaw {
    /// Create a Burke-Shaw attractor with default parameters (σ=10, ρ=4.272).
    pub fn new() -> Self {
        Self {
            state: vec![0.6, 0.0, 0.0],
            sigma: 10.0,
            rho: 4.272,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], sigma: f64, rho: f64) -> Vec<f64> {
        vec![
            -sigma * (s[0] + s[1]),
            -s[1] - sigma * s[0] * s[2],
            sigma * s[0] * s[1] + rho,
        ]
    }
}

impl Default for BurkeShaw {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for BurkeShaw {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "burke_shaw"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.sigma, self.rho)
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
        let (sigma, rho) = (self.sigma, self.rho);
        rk4(&mut self.state, dt, |s| Self::deriv(s, sigma, rho));
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
    fn test_burke_shaw_initial_state() {
        let sys = BurkeShaw::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "burke_shaw");
        let s = sys.state();
        assert!(s.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_burke_shaw_step_changes_state() {
        let mut sys = BurkeShaw::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_burke_shaw_state_stays_finite() {
        let mut sys = BurkeShaw::new();
        for _ in 0..1000 {
            sys.step(0.01);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_burke_shaw_deterministic() {
        let mut s1 = BurkeShaw::new();
        let mut s2 = BurkeShaw::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_burke_shaw_set_state() {
        let mut sys = BurkeShaw::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_burke_shaw_deriv_at_known_point() {
        // At (1, 0, 0) with sigma=10, rho=4.272:
        // dx = -10*(1+0) = -10, dy = 0 - 10*1*0 = 0, dz = 10*1*0 + 4.272 = 4.272
        let sys = BurkeShaw::new();
        let d = sys.deriv_at(&[1.0, 0.0, 0.0]);
        assert!((d[0] - (-10.0)).abs() < 1e-12, "d[0]={}", d[0]);
        assert!(d[1].abs() < 1e-12, "d[1]={}", d[1]);
        assert!((d[2] - 4.272).abs() < 1e-12, "d[2]={}", d[2]);
    }
}
