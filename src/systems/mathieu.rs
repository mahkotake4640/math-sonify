use super::{rk4, DynamicalSystem};

/// Mathieu equation — parametric resonance.
///
/// 2D oscillator with time-varying stiffness:
///   dx/dt = v
///   dv/dt = -(a + 2*q*cos(2*t)) * x
///
/// The internal time t is carried as state[2] and advances each step.
/// `dimension()` returns 3 (including internal time) but the useful dimensions
/// for sonification are state[0] (displacement) and state[1] (velocity).
pub struct Mathieu {
    /// [x, v, t_internal]
    state: Vec<f64>,
    pub a: f64,
    pub q: f64,
    speed: f64,
}

impl Mathieu {
    pub fn new(a: f64, q: f64) -> Self {
        Self {
            state: vec![1.0, 0.0, 0.0],
            a,
            q,
            speed: 0.0,
        }
    }

    fn deriv(state: &[f64], a: f64, q: f64) -> Vec<f64> {
        let x = state[0];
        let v = state[1];
        let t = state[2];
        vec![
            v,
            -(a + 2.0 * q * (2.0 * t).cos()) * x,
            1.0, // dt/dt = 1
        ]
    }
}

impl DynamicalSystem for Mathieu {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "Mathieu"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.q)
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
        let (a, q) = (self.a, self.q);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |st| Self::deriv(st, a, q));
        let ds: f64 = self.state[0..2]
            .iter()
            .zip(prev[0..2].iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        self.speed = ds / dt.max(1e-15);
    }
}
