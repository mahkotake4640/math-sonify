use super::DynamicalSystem;

/// Hénon map — a discrete-time two-dimensional chaotic map.
///
/// Iteration rule (a=1.4, b=0.3 default):
/// ```text
/// x_{n+1} = 1 - a·x² + y
/// y_{n+1} = b·x
/// ```
/// The strange attractor of the Hénon map has fractal dimension ≈ 1.26.
pub struct HenonMap {
    state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    speed: f64,
}

impl HenonMap {
    /// Create a Hénon map with default parameters (a=1.4, b=0.3).
    pub fn new() -> Self {
        Self {
            state: vec![0.0, 0.0, 0.0],
            a: 1.4,
            b: 0.3,
            speed: 0.0,
        }
    }
}

impl DynamicalSystem for HenonMap {
    fn state(&self) -> &[f64] { &self.state }
    fn dimension(&self) -> usize { 3 }
    fn name(&self) -> &str { "henon_map" }
    fn speed(&self) -> f64 { self.speed }

    fn deriv_at(&self, _state: &[f64]) -> Vec<f64> {
        vec![0.0; 3]
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() { self.state[i] = s[i]; }
        }
    }

    fn step(&mut self, dt: f64) {
        let x = self.state[0];
        let y = self.state[1];
        let new_x = 1.0 - self.a * x * x + y;
        let new_y = self.b * x;
        let delta = (new_x - x).abs();
        self.speed = if dt > 0.0 { delta / dt } else { delta };
        self.state[0] = new_x;
        self.state[1] = new_y;
        // state[2] stays 0.0
    }
}
