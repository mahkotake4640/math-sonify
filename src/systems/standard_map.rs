use super::DynamicalSystem;
use std::f64::consts::TAU;

/// Chirikov–Taylor standard map — an area-preserving 2D discrete map.
///
/// Iteration rule (all angles in [0, 2π)):
/// ```text
/// p_{n+1} = (p + k · sin θ) mod 2π
/// θ_{n+1} = (θ + p_{n+1}) mod 2π
/// ```
/// For k = 0 the motion is integrable (circles in phase space).
/// For k ≈ 0.97 the last KAM torus breaks, yielding global chaos.
/// For large k the phase space is almost entirely chaotic.
///
/// The state is (θ, p, k) so the sonification pipeline gets both
/// the angle and the momentum as independent signal dimensions.
pub struct StandardMap {
    state: Vec<f64>,
    pub k: f64,
    speed: f64,
}

impl StandardMap {
    /// Create a standard map with the given stochasticity parameter k.
    /// Default k = 1.5 gives strong but not complete chaos.
    pub fn new(k: f64) -> Self {
        Self {
            state: vec![0.5, 0.5, k],
            k,
            speed: 0.0,
        }
    }
}

impl DynamicalSystem for StandardMap {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        3
    }

    fn name(&self) -> &str {
        "standard_map"
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
        let theta = self.state[0];
        let p = self.state[1];

        let new_p = (p + self.k * theta.sin()).rem_euclid(TAU);
        let new_theta = (theta + new_p).rem_euclid(TAU);

        let delta = ((new_theta - theta).powi(2) + (new_p - p).powi(2)).sqrt();
        self.speed = delta;

        self.state[0] = new_theta;
        self.state[1] = new_p;
        self.state[2] = self.k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;
    use std::f64::consts::TAU;

    #[test]
    fn test_standard_map_initial_state() {
        let sys = StandardMap::new(1.5);
        let s = sys.state();
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.5).abs() < 1e-15);
        assert!((s[1] - 0.5).abs() < 1e-15);
        assert!((s[2] - 1.5).abs() < 1e-15);
        assert_eq!(sys.name(), "standard_map");
        assert_eq!(sys.dimension(), 3);
    }

    #[test]
    fn test_standard_map_step_changes_state() {
        let mut sys = StandardMap::new(1.5);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.01);
        let after = sys.state();
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15),
            "State did not change after step"
        );
    }

    #[test]
    fn test_standard_map_stays_in_torus() {
        let mut sys = StandardMap::new(1.5);
        for _ in 0..1000 {
            sys.step(0.01);
            let theta = sys.state()[0];
            let p = sys.state()[1];
            assert!(theta >= 0.0 && theta < TAU, "theta out of [0, 2π): {}", theta);
            assert!(p >= 0.0 && p < TAU, "p out of [0, 2π): {}", p);
        }
    }

    #[test]
    fn test_standard_map_deterministic() {
        let mut sys1 = StandardMap::new(1.5);
        let mut sys2 = StandardMap::new(1.5);
        for _ in 0..200 {
            sys1.step(0.01);
            sys2.step(0.01);
        }
        for (a, b) in sys1.state().iter().zip(sys2.state().iter()) {
            assert!((a - b).abs() < 1e-15, "Non-deterministic: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_standard_map_k_zero_integrable() {
        // With k=0 the map becomes p_{n+1} = p, θ_{n+1} = θ + p (mod 2π).
        let mut sys = StandardMap::new(0.0);
        let p0 = sys.state()[1];
        let theta0 = sys.state()[0];
        sys.step(0.0);
        let theta1 = sys.state()[0];
        let expected = (theta0 + p0).rem_euclid(TAU);
        assert!((theta1 - expected).abs() < 1e-12, "k=0 should give theta += p");
    }

    #[test]
    fn test_standard_map_speed_positive_after_step() {
        let mut sys = StandardMap::new(1.0);
        sys.step(0.01);
        assert!(sys.speed() > 0.0, "speed should be positive: {}", sys.speed());
    }

    #[test]
    fn test_standard_map_large_k_different_from_small_k() {
        // Larger k (more stochastic) should give a different trajectory than small k
        let mut sys_small = StandardMap::new(0.1);
        let mut sys_large = StandardMap::new(5.0);
        for _ in 0..100 {
            sys_small.step(0.01);
            sys_large.step(0.01);
        }
        let d: f64 = sys_small.state().iter().zip(sys_large.state().iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(d > 1e-6, "Different k should give different dynamics: d={}", d);
    }

    #[test]
    fn test_standard_map_area_preserving() {
        // The Chirikov-Taylor map is symplectic: det(Jacobian) = 1.
        // We verify this numerically: a tiny parallelogram in (θ, p) phase space
        // has the same area before and after one step of the map.
        //
        // One step: p' = p + k·sinθ,  θ' = θ + p'
        // Jacobian: [[∂θ'/∂θ, ∂θ'/∂p], [∂p'/∂θ, ∂p'/∂p]]
        //         = [[1+k·cosθ, 1], [k·cosθ, 1]]
        // det = (1+k·cosθ)·1 - 1·k·cosθ = 1  ✓
        //
        // Here we test it numerically with finite differences.
        let k = 1.5f64;
        let theta0 = 1.2f64;
        let p0 = 0.8f64;
        let eps = 1e-6;

        let one_step = |theta: f64, p: f64| -> (f64, f64) {
            let new_p = (p + k * theta.sin()).rem_euclid(TAU);
            let new_theta = (theta + new_p).rem_euclid(TAU);
            (new_theta, new_p)
        };

        let (theta0_mapped, p0_mapped) = one_step(theta0, p0);
        let (theta_dtheta, p_dtheta) = one_step(theta0 + eps, p0);
        let (theta_dp, p_dp) = one_step(theta0, p0 + eps);

        // Jacobian columns (unnormalized by eps)
        let j00 = (theta_dtheta - theta0_mapped) / eps; // ∂θ'/∂θ
        let j10 = (p_dtheta - p0_mapped) / eps;         // ∂p'/∂θ
        let j01 = (theta_dp - theta0_mapped) / eps;     // ∂θ'/∂p
        let j11 = (p_dp - p0_mapped) / eps;             // ∂p'/∂p

        // Analytic Jacobian determinant = 1 for all (θ, k)
        let analytic_det = 1.0 + k * theta0.cos() - k * theta0.cos();
        assert!(
            (analytic_det - 1.0).abs() < 1e-12,
            "Analytic Jacobian determinant should be 1, got {}",
            analytic_det
        );

        // Numerical Jacobian: be careful about angle wrapping — for interior points
        // the finite-difference should also give ~1.
        // Use a point away from the wrapping boundary: theta0=1.2 rad is safe.
        let det_numerical = j00 * j11 - j01 * j10;
        assert!(
            (det_numerical - 1.0).abs() < 1e-5,
            "Numerical Jacobian determinant should be ~1, got {}",
            det_numerical
        );
    }
}
