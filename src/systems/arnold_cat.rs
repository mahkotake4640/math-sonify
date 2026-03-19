use super::DynamicalSystem;

/// Arnold cat map — a hyperbolic toral automorphism on [0,1)².
///
/// Iteration rule:
/// ```text
/// x_{n+1} = (x + y)       mod 1
/// y_{n+1} = (x + 2·y)     mod 1
/// ```
/// This is a linear map on the 2-torus with matrix [[1,1],[1,2]],
/// which has eigenvalues (3 ± √5)/2 (the golden ratio and its inverse).
/// It is uniformly hyperbolic, ergodic, and mixing with Lyapunov
/// exponent λ = ln((3+√5)/2) ≈ 0.962.
///
/// The map is often illustrated with its distortion of a "cat" image,
/// which eventually returns to the original (periodic, but very long period).
pub struct ArnoldCat {
    state: Vec<f64>,
    speed: f64,
}

impl ArnoldCat {
    pub fn new() -> Self {
        // Start at an irrational point to avoid fixed points
        Self {
            state: vec![0.1, 0.4, 0.0],
            speed: 0.0,
        }
    }
}

impl Default for ArnoldCat {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for ArnoldCat {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "arnold_cat"
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
        let x = self.state[0];
        let y = self.state[1];

        let new_x = (x + y).rem_euclid(1.0);
        let new_y = (x + 2.0 * y).rem_euclid(1.0);

        let delta = ((new_x - x).powi(2) + (new_y - y).powi(2)).sqrt();
        self.speed = delta;

        self.state[0] = new_x;
        self.state[1] = new_y;
        // state[2] unused; leave at 0 or accumulate something interesting
        self.state[2] = (self.state[2] + new_x - new_y).rem_euclid(1.0);
    }
}
