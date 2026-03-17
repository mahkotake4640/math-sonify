use super::{DynamicalSystem, rk4};

/// Lorenz-96: dx_i/dt = (x_{i+1} - x_{i-2})*x_{i-1} - x_i + F
/// N=8 oscillators, F=8.0, periodic boundary.
pub struct Lorenz96 {
    state: Vec<f64>,
    pub f: f64,
    pub n: usize,
    speed: f64,
}

impl Lorenz96 {
    pub fn new() -> Self {
        let n = 8;
        let f = 8.0;
        let mut state = vec![0.0f64; n];
        state[0] = 0.01;
        Self { state, f, n, speed: 0.0 }
    }

    fn deriv(s: &[f64], f_forcing: f64) -> Vec<f64> {
        let n = s.len();
        (0..n).map(|i| {
            let xm2 = s[(i + n - 2) % n];
            let xm1 = s[(i + n - 1) % n];
            let xp1 = s[(i + 1) % n];
            (xp1 - xm2) * xm1 - s[i] + f_forcing
        }).collect()
    }
}

impl DynamicalSystem for Lorenz96 {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { self.n }
    fn name(&self) -> &str { "lorenz96" }
    fn speed(&self) -> f64 { self.speed }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.f)
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() { self.state[i] = s[i]; }
        }
    }

    fn step(&mut self, dt: f64) {
        let f_forcing = self.f;
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, f_forcing));
        let ds: f64 = self.state.iter().zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
        self.speed = ds / dt;
    }
}
