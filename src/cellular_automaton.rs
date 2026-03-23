//! # Cellular Automaton Sonification
//!
//! Evolve 1-D elementary cellular automata (Rule 30, 90, 110) and 2-D
//! Conway's Game of Life, then sonify the evolving cell grid.
//!
//! ## Audio model
//!
//! Each **live cell** contributes a **partial oscillator**:
//! - Cell column → frequency (evenly spaced across a chosen scale).
//! - Cell row (generation) → amplitude envelope (decays across generations).
//! - Wolfram Class 3 rules (e.g., Rule 30) → broadband noise-like texture.
//! - Rule 110 (Class 4) → complex, structured, almost musical patterns.
//! - Conway's Life → slowly evolving polyphonic chords.
//!
//! ## Usage
//!
//! ```rust
//! use math_sonify::cellular_automaton::{CellularAutomaton, CAConfig, CARule};
//!
//! let cfg = CAConfig { width: 32, rule: CARule::Rule30, ..Default::default() };
//! let mut ca = CellularAutomaton::new(cfg);
//! let audio = ca.step_and_sonify(44100, 256);
//! assert_eq!(audio.len(), 256);
//! ```

/// Supported cellular automaton rules.
#[derive(Debug, Clone, PartialEq)]
pub enum CARule {
    /// Wolfram Rule 30: Class 3 (chaotic). Rich, noise-like audio.
    Rule30,
    /// Wolfram Rule 90: XOR rule. Sierpinski triangle; regular, harmonious.
    Rule90,
    /// Wolfram Rule 110: Class 4 (complex). Structured, almost musical.
    Rule110,
    /// Conway's Game of Life (2-D). Evolving polyphonic chords.
    ConwayLife,
}

impl Default for CARule {
    fn default() -> Self {
        Self::Rule30
    }
}

/// Configuration for cellular automaton sonification.
#[derive(Debug, Clone)]
pub struct CAConfig {
    /// Grid width (number of cells / columns).
    pub width: usize,
    /// Grid height for Conway's Life (rows); ignored for 1-D rules.
    pub height: usize,
    /// Which rule to evolve.
    pub rule: CARule,
    /// Base frequency (Hz) for the leftmost / lowest cell.
    pub base_frequency: f64,
    /// Frequency ratio between adjacent cells (e.g., 2^(1/12) for semitones).
    pub frequency_ratio: f64,
    /// Per-partial amplitude envelope decay per generation.
    pub amplitude_decay: f64,
}

impl Default for CAConfig {
    fn default() -> Self {
        Self {
            width: 32,
            height: 32,
            rule: CARule::Rule30,
            base_frequency: 110.0,
            frequency_ratio: 2.0_f64.powf(1.0 / 12.0), // chromatic semitone
            amplitude_decay: 0.85,
        }
    }
}

/// Sonification state for one partial.
#[derive(Debug, Clone)]
struct PartialState {
    frequency: f64,
    amplitude: f64,
    phase: f64,
    /// How many generations this cell has been alive.
    age: u32,
}

/// Cellular automaton sonifier.
pub struct CellularAutomaton {
    pub config: CAConfig,
    /// Current 1-D row state (for elementary CA).
    row: Vec<u8>,
    /// 2-D grid for Conway's Life (row-major, height × width).
    grid: Vec<Vec<u8>>,
    /// Generation counter.
    pub generation: u64,
    /// Per-column partial states.
    partials: Vec<PartialState>,
}

impl CellularAutomaton {
    /// Create a new automaton with a single live cell in the centre.
    pub fn new(config: CAConfig) -> Self {
        let w = config.width;
        let h = config.height;

        let mut row = vec![0u8; w];
        row[w / 2] = 1; // seed: single live cell

        // Conway Life: glider seed in top-left corner
        let mut grid = vec![vec![0u8; w]; h];
        if w >= 5 && h >= 5 {
            // Glider pattern
            let seed = [(0, 1), (1, 2), (2, 0), (2, 1), (2, 2)];
            for (r, c) in seed {
                grid[r][c] = 1;
            }
        } else {
            grid[0][0] = 1;
        }

        let base = config.base_frequency;
        let ratio = config.frequency_ratio;
        let partials: Vec<PartialState> = (0..w)
            .map(|i| PartialState {
                frequency: base * ratio.powi(i as i32),
                amplitude: 0.0,
                phase: 0.0,
                age: 0,
            })
            .collect();

        Self {
            config,
            row,
            grid,
            generation: 0,
            partials,
        }
    }

    /// Advance the automaton by one generation and update partial amplitudes.
    pub fn step(&mut self) {
        match self.config.rule {
            CARule::Rule30 => self.step_elementary(30),
            CARule::Rule90 => self.step_elementary(90),
            CARule::Rule110 => self.step_elementary(110),
            CARule::ConwayLife => self.step_life(),
        }
        self.generation += 1;
        self.update_partials();
    }

    /// Advance and return one block of audio samples.
    pub fn step_and_sonify(&mut self, sample_rate: u32, num_samples: usize) -> Vec<f32> {
        self.step();
        self.synthesise(sample_rate, num_samples)
    }

    /// Synthesise `num_samples` samples from current partial states.
    pub fn synthesise(&mut self, sample_rate: u32, num_samples: usize) -> Vec<f32> {
        let sr = sample_rate as f64;
        (0..num_samples)
            .map(|_| {
                let s: f64 = self.partials.iter_mut().map(|p| {
                    let out = p.amplitude * p.phase.sin();
                    p.phase += 2.0 * std::f64::consts::PI * p.frequency / sr;
                    if p.phase > std::f64::consts::TAU {
                        p.phase -= std::f64::consts::TAU;
                    }
                    out
                }).sum();
                s.tanh() as f32
            })
            .collect()
    }

    /// ASCII art rendering of the current generation.
    pub fn ascii_grid(&self) -> String {
        match self.config.rule {
            CARule::ConwayLife => self
                .grid
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|&c| if c == 1 { '█' } else { '·' })
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => self
                .row
                .iter()
                .map(|&c| if c == 1 { '█' } else { ' ' })
                .collect(),
        }
    }

    /// Which columns are currently live.
    pub fn live_columns(&self) -> Vec<usize> {
        match self.config.rule {
            CARule::ConwayLife => {
                // Aggregate across all rows
                (0..self.config.width)
                    .filter(|&c| self.grid.iter().any(|row| row[c] == 1))
                    .collect()
            }
            _ => self
                .row
                .iter()
                .enumerate()
                .filter(|(_, &v)| v == 1)
                .map(|(i, _)| i)
                .collect(),
        }
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn step_elementary(&mut self, rule: u8) {
        let w = self.config.width;
        let mut next = vec![0u8; w];
        for i in 0..w {
            let left = if i == 0 { self.row[w - 1] } else { self.row[i - 1] };
            let center = self.row[i];
            let right = if i == w - 1 { self.row[0] } else { self.row[i + 1] };
            let pattern = (left << 2) | (center << 1) | right;
            next[i] = (rule >> pattern) & 1;
        }
        self.row = next;
    }

    fn step_life(&mut self) {
        let h = self.config.height;
        let w = self.config.width;
        let mut next = vec![vec![0u8; w]; h];
        for r in 0..h {
            for c in 0..w {
                let neighbours: u32 = [
                    ((r + h - 1) % h, (c + w - 1) % w),
                    ((r + h - 1) % h, c),
                    ((r + h - 1) % h, (c + 1) % w),
                    (r, (c + w - 1) % w),
                    (r, (c + 1) % w),
                    ((r + 1) % h, (c + w - 1) % w),
                    ((r + 1) % h, c),
                    ((r + 1) % h, (c + 1) % w),
                ]
                .iter()
                .map(|&(nr, nc)| self.grid[nr][nc] as u32)
                .sum();
                next[r][c] = match (self.grid[r][c], neighbours) {
                    (1, 2) | (1, 3) => 1,
                    (0, 3) => 1,
                    _ => 0,
                };
            }
        }
        self.grid = next;
    }

    fn update_partials(&mut self) {
        let decay = self.config.amplitude_decay;
        match self.config.rule {
            CARule::ConwayLife => {
                for (c, partial) in self.partials.iter_mut().enumerate() {
                    let live = self.grid.iter().any(|row| row[c] == 1);
                    if live {
                        partial.age += 1;
                        partial.amplitude = (partial.amplitude + 0.2).min(1.0);
                    } else {
                        partial.age = 0;
                        partial.amplitude *= decay;
                    }
                }
            }
            _ => {
                for (c, partial) in self.partials.iter_mut().enumerate() {
                    if self.row[c] == 1 {
                        partial.age += 1;
                        partial.amplitude = (partial.amplitude + 0.3).min(1.0);
                    } else {
                        partial.age = 0;
                        partial.amplitude *= decay;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule30_evolves() {
        let mut ca = CellularAutomaton::new(CAConfig {
            width: 16,
            rule: CARule::Rule30,
            ..Default::default()
        });
        let initial = ca.row.clone();
        ca.step();
        assert_ne!(ca.row, initial, "Rule 30 should change state after one step");
    }

    #[test]
    fn rule90_sierpinski_symmetry() {
        let mut ca = CellularAutomaton::new(CAConfig {
            width: 16,
            rule: CARule::Rule90,
            ..Default::default()
        });
        ca.step();
        // Rule 90 from a single centre cell should produce symmetric pattern
        let mid = 8;
        for i in 0..mid {
            assert_eq!(
                ca.row[mid - 1 - i],
                ca.row[mid + i],
                "Rule 90 should be symmetric at offset {i}"
            );
        }
    }

    #[test]
    fn life_glider_survives() {
        let mut ca = CellularAutomaton::new(CAConfig {
            width: 16,
            height: 16,
            rule: CARule::ConwayLife,
            ..Default::default()
        });
        // Glider should have some live cells after several steps
        for _ in 0..10 {
            ca.step();
        }
        let live: u32 = ca.grid.iter().flat_map(|r| r.iter()).map(|&c| c as u32).sum();
        assert!(live > 0, "Glider should still be alive after 10 steps");
    }

    #[test]
    fn synthesise_output_length() {
        let mut ca = CellularAutomaton::new(CAConfig::default());
        let audio = ca.step_and_sonify(44100, 512);
        assert_eq!(audio.len(), 512);
    }

    #[test]
    fn synthesise_output_clipped() {
        let mut ca = CellularAutomaton::new(CAConfig::default());
        for _ in 0..5 {
            ca.step();
        }
        let audio = ca.synthesise(44100, 256);
        for s in &audio {
            assert!(s.abs() <= 1.0 + 1e-6, "sample out of tanh range: {s}");
        }
    }

    #[test]
    fn live_columns_subset_of_width() {
        let mut ca = CellularAutomaton::new(CAConfig::default());
        ca.step();
        let live = ca.live_columns();
        assert!(live.iter().all(|&c| c < ca.config.width));
    }

    #[test]
    fn generation_increments() {
        let mut ca = CellularAutomaton::new(CAConfig::default());
        assert_eq!(ca.generation, 0);
        ca.step();
        assert_eq!(ca.generation, 1);
        ca.step();
        assert_eq!(ca.generation, 2);
    }
}
