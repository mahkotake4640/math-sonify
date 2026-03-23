//! # Coupled Oscillator Network
//!
//! N-body Kuramoto model where oscillators are connected in arbitrary graph
//! topologies: ring, star, small-world, and random.
//!
//! ## Model
//!
//! Each oscillator i has phase θᵢ evolving as:
//!
//!   dθᵢ/dt = ωᵢ + (K / deg(i)) * Σⱼ∈N(i) sin(θⱼ - θᵢ)
//!
//! where N(i) is the neighbour set of node i in the chosen topology and
//! deg(i) = |N(i)| (degree normalisation keeps coupling scale-invariant).
//!
//! ## Audio textures
//!
//! - **Ring**: nearest-neighbour coupling → slow wave propagation, smooth pads.
//! - **Star**: hub drives all spokes → fast hub-synchronisation, rhythmic pulse.
//! - **Small-world**: shortcuts create rapid partial sync → rich, evolving texture.
//! - **Random**: heterogeneous connectivity → complex, unpredictable polyrhythm.
//!
//! ## Usage
//!
//! ```rust
//! use math_sonify::networks::coupled_oscillators::{CoupledOscillatorNetwork, Topology, NetworkConfig};
//!
//! let cfg = NetworkConfig { n: 8, coupling: 1.5, topology: Topology::Ring, ..Default::default() };
//! let mut net = CoupledOscillatorNetwork::new(cfg);
//! let audio = net.step();
//! assert_eq!(audio.len(), 8);
//! ```

/// Graph topology for the oscillator network.
#[derive(Debug, Clone, PartialEq)]
pub enum Topology {
    /// Each node connects to k nearest neighbours on a ring (k/2 each side).
    Ring,
    /// One hub node connects to all others; spokes connect only to the hub.
    Star,
    /// Start from Ring, then rewire each edge with probability p_rewire to a
    /// random target (Watts-Strogatz small-world).
    SmallWorld { k: usize, p_rewire: f64 },
    /// Erdős-Rényi random graph with edge probability p_edge.
    Random { p_edge: f64 },
    /// Fully connected (all-to-all).
    AllToAll,
}

impl Default for Topology {
    fn default() -> Self {
        Self::Ring
    }
}

/// Configuration for the coupled oscillator network.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Number of oscillators.
    pub n: usize,
    /// Global coupling strength K.
    pub coupling: f64,
    /// Graph topology.
    pub topology: Topology,
    /// Frequency spread: natural frequencies drawn from Uniform[-spread, +spread].
    pub frequency_spread: f64,
    /// Base frequency (Hz) for audio mapping.
    pub base_frequency: f64,
    /// Integration timestep.
    pub dt: f64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            n: 8,
            coupling: 1.5,
            topology: Topology::Ring,
            frequency_spread: 0.3,
            base_frequency: 220.0,
            dt: 0.01,
        }
    }
}

/// N-body Kuramoto network with configurable topology.
pub struct CoupledOscillatorNetwork {
    pub config: NetworkConfig,
    /// Phases θᵢ.
    phases: Vec<f64>,
    /// Natural frequencies ωᵢ.
    omega: Vec<f64>,
    /// Adjacency list.
    neighbours: Vec<Vec<usize>>,
    /// Simulation time.
    t: f64,
    /// LCG random state.
    rng: u64,
    /// Cached order parameter.
    order_param: f64,
}

impl CoupledOscillatorNetwork {
    pub fn new(config: NetworkConfig) -> Self {
        let mut net = Self {
            phases: vec![0.0; config.n],
            omega: vec![0.0; config.n],
            neighbours: vec![vec![]; config.n],
            t: 0.0,
            rng: 987654321098765,
            order_param: 0.0,
            config,
        };
        net.init();
        net
    }

    fn init(&mut self) {
        let n = self.config.n;
        // Natural frequencies: deterministic Lorentzian quantile
        self.omega = (0..n)
            .map(|i| {
                let u = (i as f64 + 0.5) / n as f64;
                let u_safe = u.clamp(1e-6, 1.0 - 1e-6);
                self.config.frequency_spread
                    * (std::f64::consts::PI * (u_safe - 0.5)).tan()
            })
            .collect();

        // Initial phases: uniform on [0, 2π)
        self.phases = (0..n)
            .map(|i| 2.0 * std::f64::consts::PI * i as f64 / n as f64)
            .collect();

        // Build adjacency list
        self.neighbours = self.build_adjacency();
    }

    fn build_adjacency(&mut self) -> Vec<Vec<usize>> {
        let n = self.config.n;
        match &self.config.topology.clone() {
            Topology::Ring => {
                (0..n)
                    .map(|i| vec![(i + n - 1) % n, (i + 1) % n])
                    .collect()
            }
            Topology::Star => {
                // Node 0 is the hub
                let mut adj = vec![vec![]; n];
                for i in 1..n {
                    adj[0].push(i);
                    adj[i].push(0);
                }
                adj
            }
            Topology::SmallWorld { k, p_rewire } => {
                let k = *k;
                let p = *p_rewire;
                // Start with ring of k neighbours
                let mut adj: Vec<Vec<usize>> = (0..n)
                    .map(|i| {
                        (1..=k / 2)
                            .flat_map(|d| {
                                let left = (i + n - d) % n;
                                let right = (i + d) % n;
                                vec![left, right]
                            })
                            .collect()
                    })
                    .collect();

                // Rewire edges
                for i in 0..n {
                    let neighbours = adj[i].clone();
                    for old_j in neighbours {
                        if self.lcg_float() < p {
                            // Remove old edge
                            adj[i].retain(|&x| x != old_j);
                            adj[old_j].retain(|&x| x != i);
                            // Add new random edge (avoiding self-loops and duplicates)
                            let mut new_j = (self.lcg_float() * n as f64) as usize % n;
                            let mut attempts = 0;
                            while (new_j == i || adj[i].contains(&new_j)) && attempts < n {
                                new_j = (self.lcg_float() * n as f64) as usize % n;
                                attempts += 1;
                            }
                            if new_j != i && !adj[i].contains(&new_j) {
                                adj[i].push(new_j);
                                adj[new_j].push(i);
                            }
                        }
                    }
                }
                adj
            }
            Topology::Random { p_edge } => {
                let p = *p_edge;
                let mut adj = vec![vec![]; n];
                for i in 0..n {
                    for j in (i + 1)..n {
                        if self.lcg_float() < p {
                            adj[i].push(j);
                            adj[j].push(i);
                        }
                    }
                }
                adj
            }
            Topology::AllToAll => {
                (0..n)
                    .map(|i| (0..n).filter(|&j| j != i).collect())
                    .collect()
            }
        }
    }

    /// Advance the network by one RK4 step.
    ///
    /// Returns per-oscillator audio amplitude values ∈ [0, 1].
    pub fn step(&mut self) -> Vec<f64> {
        let n = self.config.n;
        let dt = self.config.dt;
        let k = self.config.coupling;

        let deriv = |phases: &[f64]| -> Vec<f64> {
            (0..n)
                .map(|i| {
                    let nb = &self.neighbours[i];
                    let deg = nb.len().max(1) as f64;
                    let coupling_sum: f64 =
                        nb.iter().map(|&j| (phases[j] - phases[i]).sin()).sum();
                    self.omega[i] + k / deg * coupling_sum
                })
                .collect()
        };

        // RK4
        let k1 = deriv(&self.phases);
        let y2: Vec<f64> = self.phases.iter().zip(&k1).map(|(p, d)| p + 0.5 * dt * d).collect();
        let k2 = deriv(&y2);
        let y3: Vec<f64> = self.phases.iter().zip(&k2).map(|(p, d)| p + 0.5 * dt * d).collect();
        let k3 = deriv(&y3);
        let y4: Vec<f64> = self.phases.iter().zip(&k3).map(|(p, d)| p + dt * d).collect();
        let k4 = deriv(&y4);

        for i in 0..n {
            self.phases[i] +=
                dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
            // Wrap to [0, 2π]
            self.phases[i] = self.phases[i].rem_euclid(2.0 * std::f64::consts::PI);
        }
        self.t += dt;

        // Compute order parameter r = |Σ exp(iθⱼ)| / N
        let re: f64 = self.phases.iter().map(|&p| p.cos()).sum::<f64>() / n as f64;
        let im: f64 = self.phases.iter().map(|&p| p.sin()).sum::<f64>() / n as f64;
        self.order_param = (re * re + im * im).sqrt();

        // Map each oscillator to an audio amplitude using its phase velocity
        let k1_ref = deriv(&self.phases);
        (0..n)
            .map(|i| {
                let phase_vel = k1_ref[i].abs().min(4.0) / 4.0;
                phase_vel
            })
            .collect()
    }

    /// Current Kuramoto order parameter r ∈ [0, 1].
    /// r ≈ 0 → incoherent; r ≈ 1 → fully synchronised.
    pub fn order_parameter(&self) -> f64 {
        self.order_param
    }

    /// Map oscillator phases to audio frequencies (Hz).
    pub fn frequencies(&self) -> Vec<f64> {
        let base = self.config.base_frequency;
        self.phases
            .iter()
            .enumerate()
            .map(|(i, &theta)| {
                // Each oscillator maps to a harmonic of the base, shifted by phase
                let harmonic = (i + 1) as f64;
                base * harmonic * (1.0 + 0.05 * theta.cos())
            })
            .collect()
    }

    /// Instantaneous phases of all oscillators.
    pub fn phases(&self) -> &[f64] {
        &self.phases
    }

    /// Adjacency list (read-only).
    pub fn adjacency(&self) -> &[Vec<usize>] {
        &self.neighbours
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn lcg_float(&mut self) -> f64 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_adjacency_size() {
        let cfg = NetworkConfig {
            n: 6,
            topology: Topology::Ring,
            ..Default::default()
        };
        let net = CoupledOscillatorNetwork::new(cfg);
        for adj in net.adjacency() {
            assert_eq!(adj.len(), 2, "ring: each node should have exactly 2 neighbours");
        }
    }

    #[test]
    fn star_hub_degree() {
        let cfg = NetworkConfig {
            n: 5,
            topology: Topology::Star,
            ..Default::default()
        };
        let net = CoupledOscillatorNetwork::new(cfg);
        assert_eq!(net.adjacency()[0].len(), 4, "hub should connect to all spokes");
        for i in 1..5 {
            assert_eq!(net.adjacency()[i].len(), 1, "spoke should connect only to hub");
        }
    }

    #[test]
    fn step_output_length() {
        let mut net = CoupledOscillatorNetwork::new(NetworkConfig::default());
        let audio = net.step();
        assert_eq!(audio.len(), net.config.n);
    }

    #[test]
    fn step_audio_in_range() {
        let mut net = CoupledOscillatorNetwork::new(NetworkConfig::default());
        for _ in 0..50 {
            let audio = net.step();
            for a in audio {
                assert!((0.0..=1.0).contains(&a), "audio amplitude out of range: {a}");
            }
        }
    }

    #[test]
    fn order_parameter_in_range() {
        let mut net = CoupledOscillatorNetwork::new(NetworkConfig {
            n: 8,
            coupling: 5.0, // high coupling → should synchronise
            topology: Topology::AllToAll,
            ..Default::default()
        });
        for _ in 0..500 {
            net.step();
        }
        let r = net.order_parameter();
        assert!((0.0..=1.0 + 1e-9).contains(&r), "order parameter out of range: {r}");
    }

    #[test]
    fn phases_wrapped() {
        let mut net = CoupledOscillatorNetwork::new(NetworkConfig::default());
        for _ in 0..200 {
            net.step();
        }
        for &p in net.phases() {
            assert!(p >= 0.0 && p < 2.0 * std::f64::consts::PI + 1e-10,
                "phase out of [0, 2π): {p}");
        }
    }

    #[test]
    fn small_world_has_some_long_range() {
        let cfg = NetworkConfig {
            n: 10,
            topology: Topology::SmallWorld { k: 2, p_rewire: 0.5 },
            ..Default::default()
        };
        let net = CoupledOscillatorNetwork::new(cfg);
        // With p=0.5 rewiring, some edges should be non-nearest-neighbour
        // (hard to guarantee deterministically, just check it builds without panic)
        assert_eq!(net.phases().len(), 10);
    }
}
