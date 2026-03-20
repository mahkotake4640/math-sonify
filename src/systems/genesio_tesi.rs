use super::{rk4, DynamicalSystem};

/// Genesio-Tesi chaotic system.
///
/// Equations (Jerk form — third-order autonomous ODE):
/// ```text
/// x' = y
/// y' = z
/// z' = -c·x - b·y - a·z + x²
/// ```
///
/// Default parameters a=1.2, b=2.92, c=6.0 produce a strange attractor.
/// The x² nonlinearity is the sole source of chaos; the linear terms
/// determine damping and frequency. The system is equivalent to
/// x''' + a·x'' + b·x' + c·x = x².
///
/// Reference: Genesio, R. & Tesi, A. (1992). "Harmonic balance methods
/// for the analysis of chaotic dynamics in nonlinear systems."
/// Automatica 28(3), 531–548.
pub struct GenesioTesi {
    pub state: Vec<f64>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    speed: f64,
}

impl GenesioTesi {
    pub fn new() -> Self {
        Self {
            state: vec![0.1, 0.1, 0.1],
            a: 1.2,
            b: 2.92,
            c: 6.0,
            speed: 0.0,
        }
    }

    fn deriv(s: &[f64], a: f64, b: f64, c: f64) -> Vec<f64> {
        vec![
            s[1],
            s[2],
            -c * s[0] - b * s[1] - a * s[2] + s[0] * s[0],
        ]
    }
}

impl Default for GenesioTesi {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicalSystem for GenesioTesi {
    fn state(&self) -> &[f64] {
        &self.state
    }
    fn dimension(&self) -> usize {
        3
    }
    fn name(&self) -> &str {
        "genesio_tesi"
    }
    fn speed(&self) -> f64 {
        self.speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        Self::deriv(state, self.a, self.b, self.c)
    }

    fn step(&mut self, dt: f64) {
        let (a, b, c) = (self.a, self.b, self.c);
        let prev = self.state.clone();
        rk4(&mut self.state, dt, |s| Self::deriv(s, a, b, c));
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
    fn genesio_tesi_initial_state_finite() {
        let sys = GenesioTesi::new();
        assert!(sys.state().iter().all(|v| v.is_finite()), "Initial state has non-finite values");
    }

    #[test]
    fn genesio_tesi_stays_finite() {
        let mut sys = GenesioTesi::new();
        for _ in 0..5_000 {
            sys.step(0.005);
        }
        assert!(sys.state().iter().all(|v| v.is_finite()), "State became non-finite: {:?}", sys.state());
    }

    #[test]
    fn genesio_tesi_state_bounded() {
        let mut sys = GenesioTesi::new();
        for _ in 0..5_000 {
            sys.step(0.005);
        }
        let s = sys.state();
        // Generous bounds: attractor core is within ±2, but transient and
        // sensitivity to dt can push components up to ~5 before settling.
        assert!(s[0].abs() < 6.0, "x out of range: {}", s[0]);
        assert!(s[1].abs() < 8.0, "y out of range: {}", s[1]);
        assert!(s[2].abs() < 15.0, "z out of range: {}", s[2]);
    }

    #[test]
    fn genesio_tesi_step_changes_state() {
        let mut sys = GenesioTesi::new();
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.005);
        assert!(
            before.iter().zip(sys.state().iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn genesio_tesi_deterministic() {
        let mut s1 = GenesioTesi::new();
        let mut s2 = GenesioTesi::new();
        for _ in 0..500 {
            s1.step(0.005);
            s2.step(0.005);
        }
        for (a, b) in s1.state().iter().zip(s2.state().iter()) {
            assert!((a - b).abs() < 1e-12, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn genesio_tesi_set_state() {
        let mut sys = GenesioTesi::new();
        sys.set_state(&[1.0, -0.5, 0.3]);
        let s = sys.state();
        assert!((s[0] - 1.0).abs() < 1e-15);
        assert!((s[1] + 0.5).abs() < 1e-15);
        assert!((s[2] - 0.3).abs() < 1e-15);
    }

    #[test]
    fn genesio_tesi_deriv_at_known_point() {
        let sys = GenesioTesi::new();
        // At (1, 0, 0): x'=0, y'=0, z'=-c*1-b*0-a*0+1²=1-c=-5
        let d = sys.deriv_at(&[1.0, 0.0, 0.0]);
        assert!(d[0].abs() < 1e-14, "x' should be 0: {}", d[0]);
        assert!(d[1].abs() < 1e-14, "y' should be 0: {}", d[1]);
        let expected_z = 1.0 - sys.c; // = 1 - 6 = -5
        assert!((d[2] - expected_z).abs() < 1e-14, "z' expected {}: {}", expected_z, d[2]);
    }
}
