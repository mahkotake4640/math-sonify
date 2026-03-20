use super::{rk4, DynamicalSystem};

/// Lorenz-84 atmospheric circulation model — a three-dimensional flow used to
/// study low-order atmospheric dynamics and seasonal forcing.
///
/// Equations:
/// ```text
/// dx/dt = −a·x − y² − z² + a·F
/// dy/dt = −y + x·y − b·x·z + G
/// dz/dt = −z + b·x·y + x·z
/// ```
///
/// With a=0.25, b=4, F=8, G=1.23 the system exhibits a strange attractor that
/// models baroclinic instability.  The variable x represents the strength of the
/// symmetric westerly flow; y and z represent the amplitudes of two large-scale
/// waves.
pub struct Lorenz84 {
    state: Vec<f64>,
    /// Thermal relaxation rate. Default 0.25.
    pub a: f64,
    /// Rotational forcing. Default 4.0.
    pub b: f64,
    /// Symmetric forcing (external heating). Default 8.0.
    pub f: f64,
    /// Wave forcing (seasonal asymmetry). Default 1.23.
    pub g: f64,
    speed: f64,
}

impl Lorenz84 {
    /// Create a Lorenz-84 attractor with default parameters (a=0.25, b=4, F=8, G=1.23).
    pub fn new() -> Self {
        Self {
            state: vec![1.0, 0.0, 0.0],
            a: 0.25,
            b: 4.0,
            f: 8.0,
            g: 1.23,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64, f: f64, g: f64) -> Vec<f64> {
        vec![
            -a * s[0] - s[1] * s[1] - s[2] * s[2] + a * f,
            -s[1] + s[0] * s[1] - b * s[0] * s[2] + g,
            -s[2] + b * s[0] * s[1] + s[0] * s[2],
        ]
    }
}

impl Default for Lorenz84 {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for Lorenz84 {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "lorenz84"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b, self.f, self.g)
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
        let (a, b, f, g) = (self.a, self.b, self.f, self.g);
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b, f, g));
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
    fn test_lorenz84_initial_state() {
        let sys = Lorenz84::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "lorenz84");
        assert!(sys.state().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_lorenz84_step_changes_state() {
        let mut sys = Lorenz84::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_lorenz84_state_stays_finite() {
        let mut sys = Lorenz84::new();
        for _ in 0..5000 {
            sys.step(0.01);
        }
        for v in sys.state() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_lorenz84_deterministic() {
        let mut s1 = Lorenz84::new();
        let mut s2 = Lorenz84::new();
        for _ in 0..200 {
            s1.step(0.01);
            s2.step(0.01);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_lorenz84_set_state() {
        let mut sys = Lorenz84::new();
        sys.set_state(&[2.0, -1.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 2.0).abs() < 1e-15);
        assert!((s[1] - (-1.0)).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_lorenz84_deriv_at_known_point() {
        // At (0, 0, 0) with a=0.25, b=4, F=8, G=1.23:
        // dx = -0.25*0 - 0 - 0 + 0.25*8 = 2.0
        // dy = -0 + 0*0 - 4*0*0 + 1.23 = 1.23
        // dz = -0 + 4*0*0 + 0*0 = 0.0
        let sys = Lorenz84::new();
        let d = sys.deriv_at(&[0.0, 0.0, 0.0]);
        assert!((d[0] - 2.0).abs() < 1e-12, "d[0]={}", d[0]);
        assert!((d[1] - 1.23).abs() < 1e-12, "d[1]={}", d[1]);
        assert!(d[2].abs() < 1e-12, "d[2]={}", d[2]);
    }

    #[test]
    fn test_lorenz84_speed_positive_after_step() {
        let mut sys = Lorenz84::new();
        sys.step(0.01);
        assert!(sys.speed() > 0.0, "Speed should be positive after a step");
    }
}
