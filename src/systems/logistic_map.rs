use super::DynamicalSystem;

/// Logistic map — a classic 1D discrete chaotic map.
///
/// Iteration rule:
/// ```text
/// x_{n+1} = r · x · (1 − x)
/// ```
/// For r > 3.57 the map exhibits fully developed chaos.
/// Classic chaotic regime: r ≈ 3.9.
///
/// The state vector is 3D (x, r, 0) so the existing sonification
/// pipeline can use both the value and the bifurcation parameter.
pub struct LogisticMap {
    state: Vec<f64>,
    pub r: f64,
    speed: f64,
}

impl LogisticMap {
    /// Create a logistic map with the given r parameter.
    /// Default r=3.9 places it in the chaotic regime.
    pub fn new(r: f64) -> Self {
        Self {
            state: vec![0.5, r, 0.0],
            r,
            speed: 0.0,
        }
    }
}

impl DynamicalSystem for LogisticMap {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "logistic_map"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, _state: &[f64]) -> Vec<f64> {
        vec![0.0; 3]
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }

    fn step(&mut self, _dt: f64) {
        let x = self.state[0].clamp(0.0, 1.0);
        let new_x = self.r * x * (1.0 - x);
        let new_x = if new_x.is_finite() {
            new_x.clamp(0.0, 1.0)
        } else {
            0.5
        };
        let delta = (new_x - x).abs();
        self.speed = delta;
        self.state[0] = new_x;
        self.state[1] = self.r;
        // state[2] stays 0.0
    }
}
