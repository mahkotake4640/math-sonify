use super::{rk4, DynamicalSystem};

/// Thomas' cyclically symmetric attractor.
///
/// dx/dt = sin(y) − b·x
/// dy/dt = sin(z) − b·y
/// dz/dt = sin(x) − b·z
///
/// With b ≈ 0.208186 the system is chaotic and exhibits cyclic (x→y→z→x)
/// symmetry.  The attractor has a distinctive three-armed scroll structure.
/// Reducing b below ~0.209 gives a fully-developed strange attractor; values
/// above ~0.32 produce limit-cycle behaviour.
pub struct Thomas {
    state: Vec<f64>,
    pub b: f64,
    speed: f64,
}

impl Thomas {
    /// Create a Thomas attractor with the given dissipation parameter `b`.
    ///
    /// `b = 0.208186` produces the canonical strange attractor.
    pub fn new(b: f64) -> Self {
        Self {
            state: vec![0.1, 0.0, 0.0],
            b,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], b: f64) -> Vec<f64> {
        vec![
            s[1].sin() - b * s[0],
            s[2].sin() - b * s[1],
            s[0].sin() - b * s[2],
        ]
    }
}

impl DynamicalSystem for Thomas {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "thomas"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.b)
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
        let b = self.b;
        rk4(&mut self.state, dt, |s| Self::deriv(s, b));
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
