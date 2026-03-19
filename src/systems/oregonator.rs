use super::{rk4, DynamicalSystem};

/// Oregonator model of the Belousov-Zhabotinsky oscillating reaction.
///
/// 3D system:
///   dx/dt = s * (y - x*y + x - q*x²)
///   dy/dt = (-y - x*y + f*z) / s
///   dz/dt = w * (x - z)
///
/// Classic parameters: s=77.27, q=8.375e-6, w=0.161, f=1.0 (configurable).
/// State is clamped to [1e-12, 1e6] after each step to prevent blowup.
pub struct Oregonator {
    state: Vec<f64>,
    pub s: f64,
    pub q: f64,
    pub w: f64,
    pub f: f64,
    speed: f64,
}

impl Oregonator {
    pub fn new(f: f64) -> Self {
        Self {
            state: vec![1.0, 2.0, 3.0],
            s: 77.27,
            q: 8.375e-6,
            w: 0.161,
            f,
            speed: 0.0,
        }
    }

    fn deriv(state: &[f64], s: f64, q: f64, w: f64, f: f64) -> Vec<f64> {
        let x = state[0];
        let y = state[1];
        let z = state[2];
        vec![
            s * (y - x * y + x - q * x * x),
            (-y - x * y + f * z) / s,
            w * (x - z),
        ]
    }
}

impl DynamicalSystem for Oregonator {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "Oregonator"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.s, self.q, self.w, self.f)
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
        let (s, q, w, f) = (self.s, self.q, self.w, self.f);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |st| Self::deriv(st, s, q, w, f));
        // Clamp to avoid blowup
        for v in &mut self.state {
            *v = v.clamp(1e-12, 1e6);
        }
        let ds: f64 = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        self.speed = ds / dt.max(1e-15);
    }
}
