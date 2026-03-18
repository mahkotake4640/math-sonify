use super::{rk4, DynamicalSystem};

/// Hindmarsh-Rose neuron model.
///
/// Produces chaotic bursting: long stretches of rapid spiking interrupted by
/// silent hyperpolarized periods. The transition between bursting regimes
/// as `current_i` increases sounds like nothing else in the system roster.
///
/// dx/dt = y - a*x³ + b*x² + I - z
/// dy/dt = c - d*x² - y
/// dz/dt = r * (s*(x - x_rest) - z)
///
/// Canonical parameters: a=1, b=3, c=1, d=5, s=4, x_rest=-1.6
/// Interesting range: current_i ∈ [1.0, 5.0], r ∈ [0.001, 0.02]
pub struct HindmarshRose {
    state: Vec<f64>, // [x, y, z]
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub s: f64,
    pub x_rest: f64,
    pub r: f64,         // slow adaptation timescale
    pub current_i: f64, // external drive current — main control parameter
    speed: f64,
}

impl HindmarshRose {
    pub fn new(current_i: f64, r: f64) -> Self {
        Self {
            state: vec![0.0, -8.0, -1.6],
            a: 1.0,
            b: 3.0,
            c: 1.0,
            d: 5.0,
            s: 4.0,
            x_rest: -1.6,
            r,
            current_i,
            speed: 0.0,
        }
    }

    #[allow(clippy::similar_names, clippy::many_single_char_names)]
    fn deriv(
        s: &[f64],
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        r_: f64,
        sr: f64,
        x_rest: f64,
        i: f64,
    ) -> Vec<f64> {
        let x = s[0];
        let y = s[1];
        let z = s[2];
        vec![
            y - a * x * x * x + b * x * x + i - z,
            c - d * x * x - y,
            r_ * (sr * (x - x_rest) - z),
        ]
    }
}

impl DynamicalSystem for HindmarshRose {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "Hindmarsh-Rose"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(
            state,
            self.a,
            self.b,
            self.c,
            self.d,
            self.r,
            self.s,
            self.x_rest,
            self.current_i,
        )
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }

    #[allow(clippy::similar_names, clippy::many_single_char_names)]
    fn step(&mut self, dt: f64) {
        let (a, b, c, d, r, s, x_rest, i) = (
            self.a,
            self.b,
            self.c,
            self.d,
            self.r,
            self.s,
            self.x_rest,
            self.current_i,
        );
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |st| {
            Self::deriv(st, a, b, c, d, r, s, x_rest, i)
        });
        // Clamp x (membrane voltage) to prevent divergence
        self.state[0] = self.state[0].clamp(-5.0, 5.0);
        self.state[1] = self.state[1].clamp(-20.0, 20.0);
        self.state[2] = self.state[2].clamp(-5.0, 5.0);
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
