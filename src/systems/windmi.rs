use super::{rk4, DynamicalSystem};

/// WINDMI chaotic system.
///
/// A jerk-form model of ionospheric current-sheet disruptions (substorms).
///
/// Equations:
/// ```text
/// x' = y
/// y' = z
/// z' = -a·z - y + b - e^x
/// ```
///
/// Default parameters a=0.9, b=2.5 produce a strange attractor.
/// The exponential nonlinearity creates chaos without the polynomial
/// growth that can cause divergence in simpler jerk systems.
///
/// Reference: Horton, W., Weige, R. S., & Sprott, J. C. (2001).
/// "A simple dynamical model for substorm activity."
/// Journal of Geophysical Research, 106(A12), 28495–28508.
pub struct Windmi {
    pub state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    speed: f64,
}

impl Windmi {
    pub fn new() -> Self {
        Self {
            state: vec![0.0, -0.7, 0.0],
            a: 0.9,
            b: 2.5,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64) -> Vec<f64> {
        vec![s[1], s[2], -a * s[2] - s[1] + b - s[0].exp()]
    }
}

impl Default for Windmi {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for Windmi {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "windmi"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b)
    }

    fn step(&mut self, dt: f64) {
        let (a, b) = (self.a, self.b);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b));
        if !self.state.iter().all(|v| v.is_finite()) {
            self.state = prev;
            self.speed = 0.0;
            return;
        }
        self.speed = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
            / dt;
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn windmi_initial_state_finite() {
        let sys = Windmi::new();
        assert!(sys.state().iter().all(|v| v.is_finite()), "Initial state has non-finite values");
    }

    #[test]
    fn windmi_stays_finite() {
        let mut sys = Windmi::new();
        for _ in 0..5_000 {
            sys.step(0.01);
        }
        assert!(sys.state().iter().all(|v| v.is_finite()), "State became non-finite: {:?}", sys.state());
    }

    #[test]
    fn windmi_state_bounded() {
        let mut sys = Windmi::new();
        for _ in 0..5_000 {
            sys.step(0.01);
        }
        let s = sys.state();
        assert!(s[0].abs() < 8.0, "x out of range: {}", s[0]);
        assert!(s[1].abs() < 6.0, "y out of range: {}", s[1]);
        assert!(s[2].abs() < 10.0, "z out of range: {}", s[2]);
    }

    #[test]
    fn windmi_step_changes_state() {
        let mut sys = Windmi::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(
            before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn windmi_deterministic() {
        let mut s1 = Windmi::new();
        let mut s2 = Windmi::new();
        for _ in 0..500 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn windmi_set_state() {
        let mut sys = Windmi::new();
        sys.set_state(&[0.5, -1.0, 0.2]);
        let s = sys.state();
        assert!((s[0] - 0.5).abs() < 1e-15);
        assert!((s[1] + 1.0).abs() < 1e-15);
        assert!((s[2] - 0.2).abs() < 1e-15);
    }

    #[test]
    fn windmi_deriv_at_known_point() {
        let sys = Windmi::new();
        // At (0, 0, 0): x'=0, y'=0, z'= -a*0 - 0 + b - e^0 = b - 1 = 1.5
        let d = sys.deriv_at(&[0.0, 0.0, 0.0]);
        assert!(d[0].abs() < 1e-14, "x' should be 0: {}", d[0]);
        assert!(d[1].abs() < 1e-14, "y' should be 0: {}", d[1]);
        let expected_z = sys.b - 1.0; // = 1.5
        assert!((d[2] - expected_z).abs() < 1e-14, "z' expected {}: {}", expected_z, d[2]);
    }

    #[test]
    fn windmi_speed_positive_after_step() {
        let mut sys = Windmi::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0, "speed should be positive after a step, got {}", sys.speed());
    }
}
