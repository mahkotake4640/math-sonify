use super::{rk4, DynamicalSystem};

/// Liu chaotic system.
///
/// Equations:
/// ```text
/// x' = -a·x - e·y²
/// y' =  b·y - k·x·z
/// z' = -c·z + m·x·y
/// ```
///
/// Default parameters a=1, b=2.5, c=5, e=1, k=4, m=4 produce a
/// single-band strange attractor distinct from the Lorenz butterfly.
/// The y² term in the x-equation is the primary nonlinearity.
///
/// Reference: Liu, C., Liu, T., Liu, L. & Liu, K. (2004).
/// "A new chaotic attractor." Chaos, Solitons & Fractals 22, 1031–1038.
pub struct Liu {
    pub state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub e: f64,
    pub k: f64,
    pub m: f64,
    speed: f64,
}

impl Liu {
    pub fn new() -> Self {
        Self {
            state: vec![2.2, 2.4, 28.0],
            a: 1.0,
            b: 2.5,
            c: 5.0,
            e: 1.0,
            k: 4.0,
            m: 4.0,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64, c: f64, e: f64, k: f64, m: f64) -> Vec<f64> {
        vec![
            -a * s[0] - e * s[1] * s[1],
            b * s[1] - k * s[0] * s[2],
            -c * s[2] + m * s[0] * s[1],
        ]
    }
}

impl Default for Liu {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for Liu {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "liu"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b, self.c, self.e, self.k, self.m)
    }

    fn step(&mut self, dt: f64) {
        let (a, b, c, e, k, m) = (self.a, self.b, self.c, self.e, self.k, self.m);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b, c, e, k, m));
        if !self.state.iter().all(|v| v.is_finite()) {
            self.state = prev;
            self.speed = 0.0;
            return;
        }
        self.speed = self
            .state
            .iter()
            .zip(prev.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
            / dt;
    }

    fn set_state(&mut self, s: &[f64]) {
        let n = self.state.len().min(s.len());
        for i in 0..n {
            if s[i].is_finite() {
                self.state[i] = s[i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn liu_initial_state_finite() {
        let sys = Liu::new();
        assert!(sys.state().iter().all(|v| v.is_finite()), "Initial state has non-finite values");
    }

    #[test]
    fn liu_stays_finite() {
        let mut sys = Liu::new();
        for _ in 0..10_000 {
            sys.step(0.001);
        }
        assert!(sys.state().iter().all(|v| v.is_finite()), "State became non-finite: {:?}", sys.state());
    }

    #[test]
    fn liu_state_bounded() {
        let mut sys = Liu::new();
        for _ in 0..10_000 {
            sys.step(0.001);
        }
        let s = sys.state();
        assert!(s[0].abs() < 10.0, "x out of range: {}", s[0]);
        assert!(s[1].abs() < 15.0, "y out of range: {}", s[1]);
        assert!(s[2].abs() < 50.0, "z out of range: {}", s[2]);
    }

    #[test]
    fn liu_step_changes_state() {
        let mut sys = Liu::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.001);
        assert!(
            before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn liu_deterministic() {
        let mut s1 = Liu::new();
        let mut s2 = Liu::new();
        for _ in 0..500 {
            s1.step(0.001);
            s2.step(0.001);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn liu_set_state() {
        let mut sys = Liu::new();
        sys.set_state(&[1.0, -1.0, 5.0]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] + 1.0).abs() < 1e-15);
        assert!((s[2] - 5.0).abs() < 1e-15);
    }

    #[test]
    fn liu_deriv_at_known_point() {
        let sys = Liu::new();
        // At (1, 0, 0): x' = -a*1-e*0 = -1, y' = b*0-k*1*0 = 0, z' = -c*0+m*1*0 = 0
        let d = sys.deriv_at(&[1.0, 0.0, 0.0]);
        assert!((d[0] + sys.a).abs() < 1e-14, "x' expected {}: {}", -sys.a, d[0]);
        assert!(d[1].abs() < 1e-14, "y' expected 0: {}", d[1]);
        assert!(d[2].abs() < 1e-14, "z' expected 0: {}", d[2]);
    }
}
