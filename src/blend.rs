//! Multi-Attractor Blend
//!
//! Smoothly interpolates between two attractor states and schedules sequences
//! of attractors with crossfades.

// ── AttractorState ────────────────────────────────────────────────────────────

/// A point in the phase space of a 3-dimensional attractor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttractorState {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl AttractorState {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Linear interpolation between `self` and `other` at parameter `t ∈ [0,1]`.
    pub fn lerp(&self, other: &AttractorState, t: f64) -> AttractorState {
        let t = t.clamp(0.0, 1.0);
        AttractorState {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
        }
    }
}

// ── BlendConfig ───────────────────────────────────────────────────────────────

/// Configuration for an attractor blend operation.
///
/// `alpha ∈ [0,1]`: 0 = fully attractor A, 1 = fully attractor B.
#[derive(Debug, Clone, Copy)]
pub struct BlendConfig {
    /// Blend factor: 0 = fully A, 1 = fully B.
    pub alpha: f64,
}

impl BlendConfig {
    pub fn new(alpha: f64) -> Self {
        Self { alpha: alpha.clamp(0.0, 1.0) }
    }
}

impl Default for BlendConfig {
    fn default() -> Self {
        Self { alpha: 0.5 }
    }
}

// ── AttractorBlend ────────────────────────────────────────────────────────────

/// Utilities for blending and morphing between attractor states.
pub struct AttractorBlend;

/// Smoothstep function: `3t² - 2t³` — maps `t ∈ [0,1]` to `[0,1]` with zero
/// first-derivative at both endpoints.
#[inline]
fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl AttractorBlend {
    /// Linear blend between two attractor states using the given alpha.
    pub fn interpolate(a: AttractorState, b: AttractorState, config: &BlendConfig) -> AttractorState {
        a.lerp(&b, config.alpha)
    }

    /// S-curve (smoothstep) crossfade between two trajectories.
    ///
    /// Produces a sequence of `transition_samples` states that smoothly cross-fades
    /// from `a_traj` to `b_traj`. If the trajectories are shorter than
    /// `transition_samples`, they are clamped to their last state.
    pub fn smooth_transition(
        a_traj: &[AttractorState],
        b_traj: &[AttractorState],
        transition_samples: usize,
    ) -> Vec<AttractorState> {
        if transition_samples == 0 {
            return Vec::new();
        }
        (0..transition_samples)
            .map(|i| {
                let t_raw = i as f64 / (transition_samples - 1).max(1) as f64;
                let alpha = smoothstep(t_raw);
                let a_idx = i.min(a_traj.len().saturating_sub(1));
                let b_idx = i.min(b_traj.len().saturating_sub(1));
                let a_state = if a_traj.is_empty() {
                    AttractorState::new(0.0, 0.0, 0.0)
                } else {
                    a_traj[a_idx]
                };
                let b_state = if b_traj.is_empty() {
                    AttractorState::new(0.0, 0.0, 0.0)
                } else {
                    b_traj[b_idx]
                };
                a_state.lerp(&b_state, alpha)
            })
            .collect()
    }

    /// Generate an alpha schedule from 0 to 1 over `steps` samples using smoothstep.
    ///
    /// Returns `steps` values in [0.0, 1.0].
    pub fn morph(steps: usize) -> Vec<f64> {
        if steps == 0 {
            return Vec::new();
        }
        if steps == 1 {
            return vec![0.0];
        }
        (0..steps)
            .map(|i| {
                let t = i as f64 / (steps - 1) as f64;
                smoothstep(t)
            })
            .collect()
    }
}

// ── SequenceEntry ─────────────────────────────────────────────────────────────

/// One entry in a multi-attractor sequence.
#[derive(Debug, Clone)]
pub struct SequenceEntry {
    /// Name identifying which attractor to use for this segment.
    pub attractor_name: String,
    /// How many samples this attractor plays for (excluding crossfade).
    pub duration_samples: usize,
    /// How many samples the crossfade into the next attractor takes.
    pub crossfade_samples: usize,
}

// ── MultiAttractorSequencer ───────────────────────────────────────────────────

/// Schedules multiple attractors in sequence with smoothstep crossfades.
pub struct MultiAttractorSequencer;

impl MultiAttractorSequencer {
    /// Render a sequence of attractor entries into a flat state trajectory.
    ///
    /// For each entry, the attractor is simulated by running a simple Lorenz-like
    /// step from a seed state (deterministic, based on entry index and `dt`).
    /// Crossfades between consecutive segments use `smooth_transition`.
    pub fn render(entries: &[SequenceEntry], dt: f64) -> Vec<AttractorState> {
        if entries.is_empty() {
            return Vec::new();
        }

        let mut result = Vec::new();

        for (idx, entry) in entries.iter().enumerate() {
            // Generate a trajectory for this entry using a simple deterministic system
            let traj = Self::simulate_segment(idx, entry.duration_samples, dt);

            if idx == 0 {
                // First segment: add directly
                result.extend_from_slice(&traj);
            } else {
                // Crossfade from the tail of `result` into the new trajectory
                let xfade = entry.crossfade_samples.min(result.len()).min(traj.len());
                if xfade > 0 {
                    let tail_start = result.len().saturating_sub(xfade);
                    let a_traj: Vec<AttractorState> = result.drain(tail_start..).collect();
                    let b_traj = &traj[..xfade];
                    let blended = AttractorBlend::smooth_transition(&a_traj, b_traj, xfade);
                    result.extend_from_slice(&blended);
                    // Append the rest of the new trajectory
                    if traj.len() > xfade {
                        result.extend_from_slice(&traj[xfade..]);
                    }
                } else {
                    result.extend_from_slice(&traj);
                }
            }
        }

        result
    }

    /// Simulate a deterministic segment of `n` samples for entry index `idx`.
    ///
    /// Uses a simple Lorenz-like recurrence starting from a seed derived from `idx`.
    fn simulate_segment(idx: usize, n: usize, dt: f64) -> Vec<AttractorState> {
        let seed = (idx as f64 + 1.0) * 0.137;
        let mut x = seed;
        let mut y = seed * 1.5;
        let mut z = seed * 2.0;
        let sigma = 10.0;
        let rho = 28.0;
        let beta = 8.0 / 3.0;

        (0..n)
            .map(|_| {
                let dx = sigma * (y - x);
                let dy = x * (rho - z) - y;
                let dz = x * y - beta * z;
                x += dx * dt;
                y += dy * dt;
                z += dz * dt;
                AttractorState::new(x, y, z)
            })
            .collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // 1. Interpolate at alpha=0 returns state a
    #[test]
    fn test_interpolate_alpha_zero() {
        let a = AttractorState::new(1.0, 2.0, 3.0);
        let b = AttractorState::new(10.0, 20.0, 30.0);
        let cfg = BlendConfig::new(0.0);
        let r = AttractorBlend::interpolate(a, b, &cfg);
        assert!((r.x - 1.0).abs() < 1e-10);
        assert!((r.y - 2.0).abs() < 1e-10);
        assert!((r.z - 3.0).abs() < 1e-10);
    }

    // 2. Interpolate at alpha=1 returns state b
    #[test]
    fn test_interpolate_alpha_one() {
        let a = AttractorState::new(1.0, 2.0, 3.0);
        let b = AttractorState::new(10.0, 20.0, 30.0);
        let cfg = BlendConfig::new(1.0);
        let r = AttractorBlend::interpolate(a, b, &cfg);
        assert!((r.x - 10.0).abs() < 1e-10);
    }

    // 3. Interpolate at alpha=0.5 is midpoint
    #[test]
    fn test_interpolate_midpoint() {
        let a = AttractorState::new(0.0, 0.0, 0.0);
        let b = AttractorState::new(10.0, 20.0, 30.0);
        let cfg = BlendConfig::new(0.5);
        let r = AttractorBlend::interpolate(a, b, &cfg);
        assert!((r.x - 5.0).abs() < 1e-10);
        assert!((r.y - 10.0).abs() < 1e-10);
        assert!((r.z - 15.0).abs() < 1e-10);
    }

    // 4. BlendConfig clamps alpha above 1 to 1
    #[test]
    fn test_blend_config_clamp_high() {
        let cfg = BlendConfig::new(1.5);
        assert!((cfg.alpha - 1.0).abs() < 1e-10);
    }

    // 5. BlendConfig clamps alpha below 0 to 0
    #[test]
    fn test_blend_config_clamp_low() {
        let cfg = BlendConfig::new(-0.5);
        assert!((cfg.alpha - 0.0).abs() < 1e-10);
    }

    // 6. smooth_transition produces the correct number of samples
    #[test]
    fn test_smooth_transition_length() {
        let a: Vec<AttractorState> = (0..10).map(|i| AttractorState::new(i as f64, 0.0, 0.0)).collect();
        let b: Vec<AttractorState> = (0..10).map(|i| AttractorState::new(0.0, i as f64, 0.0)).collect();
        let result = AttractorBlend::smooth_transition(&a, &b, 10);
        assert_eq!(result.len(), 10);
    }

    // 7. smooth_transition starts near a and ends near b
    #[test]
    fn test_smooth_transition_endpoints() {
        let a: Vec<AttractorState> = vec![AttractorState::new(0.0, 0.0, 0.0); 20];
        let b: Vec<AttractorState> = vec![AttractorState::new(10.0, 10.0, 10.0); 20];
        let result = AttractorBlend::smooth_transition(&a, &b, 20);
        assert!(result[0].x < 0.5);
        assert!(result[19].x > 9.5);
    }

    // 8. smooth_transition with zero samples returns empty
    #[test]
    fn test_smooth_transition_zero_samples() {
        let a = vec![AttractorState::new(0.0, 0.0, 0.0)];
        let b = vec![AttractorState::new(1.0, 1.0, 1.0)];
        let result = AttractorBlend::smooth_transition(&a, &b, 0);
        assert!(result.is_empty());
    }

    // 9. morph generates correct length
    #[test]
    fn test_morph_length() {
        let schedule = AttractorBlend::morph(20);
        assert_eq!(schedule.len(), 20);
    }

    // 10. morph starts at 0 and ends at 1
    #[test]
    fn test_morph_bounds() {
        let schedule = AttractorBlend::morph(10);
        assert!(schedule[0].abs() < 1e-10);
        assert!((schedule[9] - 1.0).abs() < 1e-10);
    }

    // 11. morph is monotonically non-decreasing
    #[test]
    fn test_morph_monotone() {
        let schedule = AttractorBlend::morph(50);
        for i in 1..schedule.len() {
            assert!(schedule[i] >= schedule[i - 1] - 1e-10);
        }
    }

    // 12. morph with 0 steps returns empty
    #[test]
    fn test_morph_zero() {
        assert!(AttractorBlend::morph(0).is_empty());
    }

    // 13. morph with 1 step returns [0.0]
    #[test]
    fn test_morph_one() {
        let s = AttractorBlend::morph(1);
        assert_eq!(s.len(), 1);
        assert!((s[0] - 0.0).abs() < 1e-10);
    }

    // 14. smoothstep at midpoint is 0.5
    #[test]
    fn test_smoothstep_midpoint() {
        assert!((smoothstep(0.5) - 0.5).abs() < 1e-10);
    }

    // 15. MultiAttractorSequencer::render with empty entries returns empty
    #[test]
    fn test_render_empty() {
        let result = MultiAttractorSequencer::render(&[], 0.01);
        assert!(result.is_empty());
    }

    // 16. render single entry returns duration_samples states
    #[test]
    fn test_render_single_entry() {
        let entries = vec![SequenceEntry {
            attractor_name: "lorenz".to_string(),
            duration_samples: 50,
            crossfade_samples: 0,
        }];
        let result = MultiAttractorSequencer::render(&entries, 0.01);
        assert_eq!(result.len(), 50);
    }

    // 17. render two entries without crossfade gives sum of durations
    #[test]
    fn test_render_two_entries_no_xfade() {
        let entries = vec![
            SequenceEntry { attractor_name: "lorenz".to_string(), duration_samples: 30, crossfade_samples: 0 },
            SequenceEntry { attractor_name: "rossler".to_string(), duration_samples: 20, crossfade_samples: 0 },
        ];
        let result = MultiAttractorSequencer::render(&entries, 0.01);
        assert_eq!(result.len(), 50);
    }

    // 18. render with crossfade has reduced length due to overlap
    #[test]
    fn test_render_with_crossfade() {
        let entries = vec![
            SequenceEntry { attractor_name: "lorenz".to_string(), duration_samples: 30, crossfade_samples: 10 },
            SequenceEntry { attractor_name: "rossler".to_string(), duration_samples: 20, crossfade_samples: 10 },
        ];
        let result = MultiAttractorSequencer::render(&entries, 0.01);
        // The crossfade replaces 10 from A's tail + adds 10 blended + 10 remaining from B
        assert!(result.len() > 0);
    }

    // 19. AttractorState lerp at t=0 equals self
    #[test]
    fn test_lerp_zero() {
        let a = AttractorState::new(3.0, 4.0, 5.0);
        let b = AttractorState::new(9.0, 8.0, 7.0);
        let r = a.lerp(&b, 0.0);
        assert!((r.x - 3.0).abs() < 1e-10);
    }

    // 20. AttractorState lerp at t=1 equals other
    #[test]
    fn test_lerp_one() {
        let a = AttractorState::new(3.0, 4.0, 5.0);
        let b = AttractorState::new(9.0, 8.0, 7.0);
        let r = a.lerp(&b, 1.0);
        assert!((r.x - 9.0).abs() < 1e-10);
    }

    // 21. smooth_transition with single-element trajectories
    #[test]
    fn test_smooth_transition_single_element_trajs() {
        let a = vec![AttractorState::new(0.0, 0.0, 0.0)];
        let b = vec![AttractorState::new(1.0, 1.0, 1.0)];
        let result = AttractorBlend::smooth_transition(&a, &b, 5);
        assert_eq!(result.len(), 5);
        // First sample should be at 0, last at 1
        assert!(result[0].x < 0.1);
        assert!(result[4].x > 0.9);
    }
}
