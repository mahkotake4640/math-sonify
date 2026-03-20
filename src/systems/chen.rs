use super::{rk4, DynamicalSystem};

/// Chen attractor — a three-parameter chaotic system closely related to Lorenz.
///
/// Equations:
/// ```text
/// dx/dt = a·(y − x)
/// dy/dt = (c − a)·x − x·z + c·y
/// dz/dt = x·y − b·z
/// ```
///
/// With a=40, b=3, c=28 the system is chaotic and produces a double-scroll
/// attractor with richer frequency content than Lorenz.  The Chen system was
/// derived from the Lorenz system by feedback anticontrol in 1999.
pub struct Chen {
    state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    speed: f64,
}

impl Chen {
    /// Create a Chen attractor with default parameters (a=40, b=3, c=28).
    pub fn new() -> Self {
        Self {
            state: vec![-0.1, 0.5, -0.6],
            a: 40.0,
            b: 3.0,
            c: 28.0,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64, c: f64) -> Vec<f64> {
        vec![
            a * (s[1] - s[0]),
            (c - a) * s[0] - s[0] * s[2] + c * s[1],
            s[0] * s[1] - b * s[2],
        ]
    }
}

impl Default for Chen {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for Chen {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "chen"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b, self.c)
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
        let (a, b, c) = (self.a, self.b, self.c);
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b, c));
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
    fn test_chen_initial_state() {
        let sys = Chen::new();
        assert_eq!(sys.dimension(), 3);
        assert_eq!(sys.name(), "chen");
        let s = sys.state();
        assert!(s.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_chen_step_changes_state() {
        let mut sys = Chen::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.001);
        assert!(before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_chen_state_stays_finite() {
        let mut sys = Chen::new();
        for _ in 0..1000 {
            sys.step(0.001);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_chen_deterministic() {
        let mut s1 = Chen::new();
        let mut s2 = Chen::new();
        for _ in 0..200 {
            s1.step(0.001);
            s2.step(0.001);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn test_chen_set_state() {
        let mut sys = Chen::new();
        sys.set_state(&[1.0, 2.0, 3.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] - 2.0).abs() < 1e-15);
        assert!((s[2] - 3.0).abs() < 1e-15);
    }

    #[test]
    fn test_chen_deriv_at_origin() {
        // At origin: dx = 0, dy = 0, dz = 0
        let sys = Chen::new();
        let d = sys.deriv_at(&[0.0, 0.0, 0.0]);
        assert!(d[0].abs() < 1e-12);
        assert!(d[1].abs() < 1e-12);
        assert!(d[2].abs() < 1e-12);
    }

    #[test]
    fn test_chen_deriv_at_known_point() {
        // At (1, 0, 0) with a=40, b=3, c=28:
        // dx = 40*(0-1) = -40, dy = (28-40)*1 - 1*0 + 28*0 = -12, dz = 0 - 0 = 0
        let sys = Chen::new();
        let d = sys.deriv_at(&[1.0, 0.0, 0.0]);
        assert!((d[0] - (-40.0)).abs() < 1e-12, "d[0]={}", d[0]);
        assert!((d[1] - (-12.0)).abs() < 1e-12, "d[1]={}", d[1]);
        assert!(d[2].abs() < 1e-12, "d[2]={}", d[2]);
    }
}
