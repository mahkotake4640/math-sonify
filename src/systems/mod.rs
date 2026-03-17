pub mod lorenz;
pub mod custom_ode;
pub mod fractional_lorenz;
pub mod rossler;
pub mod double_pendulum;
pub mod geodesic_torus;
pub mod kuramoto;
pub mod three_body;
pub mod duffing;
pub mod van_der_pol;
pub mod halvorsen;
pub mod aizawa;
pub mod chua;
pub mod hindmarsh_rose;
pub mod coupled_map_lattice;

pub use lorenz::Lorenz;
pub use custom_ode::{CustomOde, validate_exprs};
pub use fractional_lorenz::FractionalLorenz;
pub use rossler::Rossler;
pub use double_pendulum::DoublePendulum;
pub use geodesic_torus::GeodesicTorus;
pub use kuramoto::Kuramoto;
pub use three_body::ThreeBody;
pub use duffing::Duffing;
pub use van_der_pol::VanDerPol;
pub use halvorsen::Halvorsen;
pub use aizawa::Aizawa;
pub use chua::Chua;
pub use hindmarsh_rose::HindmarshRose;
pub use coupled_map_lattice::CoupledMapLattice;

/// A continuous-time dynamical system that can be stepped forward.
pub trait DynamicalSystem: Send {
    fn state(&self) -> &[f64];
    fn step(&mut self, dt: f64);
    fn dimension(&self) -> usize;
    fn name(&self) -> &str;
    /// Approximate speed of the trajectory (|dx/dt|) — used by granular mode.
    fn speed(&self) -> f64 {
        // Default: Euclidean norm of derivative estimate from last step.
        // Systems can override with a direct formula.
        1.0
    }

    /// Compute the derivative (dx/dt) at an arbitrary state point.
    /// Used for vector field visualization.
    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        // Default: zero vector
        vec![0.0; state.len()]
    }

    /// Return the current derivative (at self.state())
    fn current_deriv(&self) -> Vec<f64> {
        self.deriv_at(self.state())
    }

    /// Load a saved state vector. Default: no-op (systems that don't override will ignore).
    fn set_state(&mut self, _s: &[f64]) {}
}

/// Runge-Kutta 4 helper. Integrates `f(state) -> derivative` by dt.
pub fn rk4<F>(state: &mut Vec<f64>, dt: f64, f: F)
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let n = state.len();
    let k1 = f(state);
    let s2: Vec<f64> = (0..n).map(|i| state[i] + 0.5 * dt * k1[i]).collect();
    let k2 = f(&s2);
    let s3: Vec<f64> = (0..n).map(|i| state[i] + 0.5 * dt * k2[i]).collect();
    let k3 = f(&s3);
    let s4: Vec<f64> = (0..n).map(|i| state[i] + dt * k3[i]).collect();
    let k4 = f(&s4);
    for i in 0..n {
        state[i] += dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
    }
}

/// Dormand-Prince RK4(5) embedded pair. Advances `state` using the 4th-order solution.
/// Returns `(rms_error, suggested_next_dt)`.
pub fn rk45_step<F>(state: &mut Vec<f64>, dt: f64, f: &F) -> (f64, f64)
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let n = state.len();
    let k1 = f(state);
    let s2: Vec<f64> = (0..n).map(|i| state[i] + dt * (1.0/5.0) * k1[i]).collect();
    let k2 = f(&s2);
    let s3: Vec<f64> = (0..n).map(|i| state[i] + dt * (3.0/40.0 * k1[i] + 9.0/40.0 * k2[i])).collect();
    let k3 = f(&s3);
    let s4: Vec<f64> = (0..n).map(|i| state[i] + dt * (44.0/45.0 * k1[i] - 56.0/15.0 * k2[i] + 32.0/9.0 * k3[i])).collect();
    let k4 = f(&s4);
    let s5: Vec<f64> = (0..n).map(|i| state[i] + dt * (19372.0/6561.0 * k1[i] - 25360.0/2187.0 * k2[i] + 64448.0/6561.0 * k3[i] - 212.0/729.0 * k4[i])).collect();
    let k5 = f(&s5);
    let s6: Vec<f64> = (0..n).map(|i| state[i] + dt * (9017.0/3168.0 * k1[i] - 355.0/33.0 * k2[i] + 46732.0/5247.0 * k3[i] + 49.0/176.0 * k4[i] - 5103.0/18656.0 * k5[i])).collect();
    let k6 = f(&s6);
    // 4th-order solution
    for i in 0..n {
        state[i] += dt * (35.0/384.0 * k1[i] + 500.0/1113.0 * k3[i] + 125.0/192.0 * k4[i]
            - 2187.0/6784.0 * k5[i] + 11.0/84.0 * k6[i]);
    }
    // 5th-order uses FSAL (k7 = f at new state)
    let k7 = f(state);
    let err: f64 = {
        let sum_sq: f64 = (0..n).map(|i| {
            let e = dt * (71.0/57600.0 * k1[i] - 71.0/16695.0 * k3[i] + 71.0/1920.0 * k4[i]
                - 17253.0/339200.0 * k5[i] + 22.0/525.0 * k6[i] - 1.0/40.0 * k7[i]);
            e * e
        }).sum();
        (sum_sq / n as f64).sqrt()
    };
    let next_dt = if err > 1e-15 {
        dt * 0.9 * (1e-6 / err).powf(0.2)
    } else {
        dt * 2.0
    };
    (err, next_dt.clamp(dt * 0.1, dt * 5.0))
}

/// Integrate `state` from 0 to `total_dt` using adaptive Dormand-Prince RK45.
/// Automatically adjusts internal step size to keep local error below `tol`.
/// Returns the number of accepted sub-steps taken.
pub fn integrate_adaptive<F>(state: &mut Vec<f64>, total_dt: f64, tol: f64, f: F) -> usize
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let mut t = 0.0;
    let mut dt = (total_dt * 0.1).min(0.01).max(total_dt * 1e-5);
    let mut steps = 0usize;
    while t < total_dt - 1e-14 {
        let remaining = total_dt - t;
        let step = dt.min(remaining);
        let mut trial = state.clone();
        let (err, next_dt) = rk45_step(&mut trial, step, &f);
        if err <= tol || step <= total_dt * 1e-7 {
            *state = trial;
            t += step;
            steps += 1;
        }
        dt = next_dt.clamp(total_dt * 1e-6, total_dt);
        if steps > 100_000 { break; }
    }
    steps
}

/// Compute up to `n_exponents` Lyapunov exponents using QR/Gram-Schmidt reorthogonalization.
///
/// - `initial_state`: starting state (should already be on the attractor)
/// - `dim`: state-space dimension
/// - `n_exponents`: number of exponents to compute (capped at `dim`)
/// - `n_steps`: integration steps for accumulation
/// - `dt`: step size
/// - `f`: derivative function dx/dt = f(x)
///
/// Returns exponents ordered largest-first.
pub fn lyapunov_spectrum<F>(
    initial_state: &[f64],
    dim: usize,
    n_exponents: usize,
    n_steps: usize,
    dt: f64,
    f: &F,
) -> Vec<f64>
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let n = n_exponents.min(dim);
    if n == 0 || dim == 0 { return Vec::new(); }
    let mut state = initial_state.to_vec();
    // Orthonormal basis for tangent space (identity columns)
    let mut q: Vec<Vec<f64>> = (0..n).map(|i| {
        let mut v = vec![0.0; dim];
        if i < dim { v[i] = 1.0; }
        v
    }).collect();
    let eps = 1e-8;
    let mut log_sum = vec![0.0f64; n];
    for _ in 0..n_steps {
        let state_old = state.clone();
        rk4(&mut state, dt, f);
        // Evolve each tangent vector via linearized flow (finite-difference Jacobian)
        for pv in &mut q {
            let mut perturbed: Vec<f64> = state_old.iter().zip(pv.iter())
                .map(|(&s, &p)| s + eps * p).collect();
            rk4(&mut perturbed, dt, f);
            for i in 0..dim {
                pv[i] = (perturbed[i] - state[i]) / eps;
            }
        }
        // QR via modified Gram-Schmidt
        for i in 0..n {
            let norm = q[i].iter().map(|&v| v * v).sum::<f64>().sqrt();
            if norm > 1e-15 {
                log_sum[i] += norm.ln();
                for j in 0..dim { q[i][j] /= norm; }
            }
            for j in (i + 1)..n {
                let dot: f64 = q[i].iter().zip(q[j].iter()).map(|(&a, &b)| a * b).sum();
                for k in 0..dim { q[j][k] -= dot * q[i][k]; }
            }
        }
    }
    let total_time = n_steps as f64 * dt;
    log_sum.iter().map(|&s| s / total_time).collect()
}

/// Symplectic (leapfrog / Störmer-Verlet) helper for Hamiltonian systems.
/// `q_indices` and `p_indices` are the indices of positions and momenta in state.
/// `dH_dq` computes -∂H/∂q (force), `dH_dp` computes ∂H/∂p (velocity).
pub fn leapfrog<Fv, Fa>(
    state: &mut Vec<f64>,
    q_idx: &[usize],
    p_idx: &[usize],
    dt: f64,
    velocity: Fv,
    force: Fa,
) where
    Fv: Fn(&[f64]) -> Vec<f64>,
    Fa: Fn(&[f64]) -> Vec<f64>,
{
    let n = q_idx.len();
    // half-kick momenta
    let f = force(state);
    for i in 0..n {
        state[p_idx[i]] += 0.5 * dt * f[i];
    }
    // full drift positions
    let v = velocity(state);
    for i in 0..n {
        state[q_idx[i]] += dt * v[i];
    }
    // half-kick momenta again
    let f2 = force(state);
    for i in 0..n {
        state[p_idx[i]] += 0.5 * dt * f2[i];
    }
}
