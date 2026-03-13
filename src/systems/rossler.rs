use super::{DynamicalSystem, rk4};

/// Rössler attractor: dx/dt = -y-z, dy/dt = x+ay, dz/dt = b+z(x-c)
pub struct Rossler {
    state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    speed: f64,
}

impl Rossler {
    pub fn new(a: f64, b: f64, c: f64) -> Self {
        Self { state: vec![1.0, 0.0, 0.0], a, b, c, speed: 0.0 }
    }

    fn deriv(s: &[f64], a: f64, b: f64, c: f64) -> Vec<f64> {
        vec![
            -s[1] - s[2],
            s[0] + a * s[1],
            b + s[2] * (s[0] - c),
        ]
    }
}

impl DynamicalSystem for Rossler {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 3 }
    fn name(&self) -> &str { "Rössler" }
    fn speed(&self) -> f64 { self.speed }
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> { Self::deriv(state, self.a, self.b, self.c) }

    fn step(&mut self, dt: f64) {
        let (a, b, c) = (self.a, self.b, self.c);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b, c));
        let ds: f64 = self.state.iter().zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
        self.speed = ds / dt;
    }
}
