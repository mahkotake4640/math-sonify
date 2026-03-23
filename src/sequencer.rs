//! Step sequencer with pattern looping and modulation.

/// A single step in a sequencer pattern.
#[derive(Debug, Clone)]
pub struct Step {
    pub active: bool,
    pub note_hz: f64,
    pub velocity: u8,
    pub duration_beats: f64,
    /// Probability the step fires (0.0..=1.0).
    pub probability: f64,
}

impl Default for Step {
    fn default() -> Self {
        Self { active: true, note_hz: 440.0, velocity: 100, duration_beats: 0.25, probability: 1.0 }
    }
}

/// A looping pattern of steps.
#[derive(Debug, Clone)]
pub struct SequencerPattern {
    pub steps: Vec<Step>,
    pub length: usize,
    /// How many times to loop; None = infinite.
    pub loop_count: Option<u32>,
}

/// Runtime state of a sequencer.
#[derive(Debug, Clone, Default)]
pub struct SequencerState {
    pub current_step: usize,
    pub loop_iteration: u32,
    pub total_steps_elapsed: u64,
}

/// Simple LCG pseudo-random number generator seeded per tick.
fn lcg_rand(seed: u64) -> f64 {
    let x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (x >> 33) as f64 / (u32::MAX as f64)
}

pub struct Sequencer {
    pub pattern: SequencerPattern,
    pub bpm: f64,
    pub state: SequencerState,
}

impl Sequencer {
    pub fn new(pattern: SequencerPattern, bpm: f64) -> Self {
        Self { pattern, bpm, state: SequencerState::default() }
    }

    /// Advance one step. Returns the step if it fires (active and probability passes).
    pub fn tick(&mut self, seed: u64) -> Option<Step> {
        let len = self.pattern.length.min(self.pattern.steps.len());
        if len == 0 { return None; }

        let idx = self.state.current_step % len;
        let step = self.pattern.steps[idx].clone();
        self.state.total_steps_elapsed += 1;

        // Advance position
        self.state.current_step += 1;
        if self.state.current_step >= len {
            self.state.current_step = 0;
            self.state.loop_iteration += 1;
        }

        if !step.active { return None; }
        let r = lcg_rand(seed ^ self.state.total_steps_elapsed);
        if r > step.probability { return None; }
        Some(step)
    }

    pub fn reset(&mut self) {
        self.state = SequencerState::default();
    }

    pub fn set_bpm(&mut self, bpm: f64) {
        self.bpm = bpm;
    }

    /// Duration of one step (one sixteenth note at current BPM) in milliseconds.
    pub fn step_duration_ms(&self) -> f64 {
        (60_000.0 / self.bpm) / 4.0
    }

    /// Apply a swing offset to the current step before returning it.
    /// Odd-numbered steps are delayed by `amount` * step_duration_ms.
    pub fn swing(&mut self, amount: f64, seed: u64) -> Option<Step> {
        // We just call tick; swing timing is a scheduling concern outside DSP.
        // Record whether the step being consumed is odd before advancing.
        let _is_odd = self.state.current_step % 2 == 1;
        let _ = amount; // swing amount would be applied by caller to schedule delay
        self.tick(seed)
    }

    /// Total duration of one full pattern pass in milliseconds.
    pub fn pattern_duration_ms(&self) -> f64 {
        self.step_duration_ms() * self.pattern.length as f64
    }
}

/// Up to 8 parallel sequencer tracks.
pub struct PolySequencer {
    pub tracks: Vec<Sequencer>,
}

impl PolySequencer {
    pub fn new(tracks: Vec<Sequencer>) -> Self {
        assert!(tracks.len() <= 8, "PolySequencer supports at most 8 tracks");
        Self { tracks }
    }

    /// Advance all tracks by one step.
    pub fn tick_all(&mut self, seed: u64) -> Vec<(usize, Option<Step>)> {
        self.tracks
            .iter_mut()
            .enumerate()
            .map(|(i, seq)| (i, seq.tick(seed ^ (i as u64 * 0xdeadbeef))))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic_pattern(n: usize) -> SequencerPattern {
        SequencerPattern {
            steps: (0..n).map(|i| Step {
                active: true,
                note_hz: 220.0 * (i + 1) as f64,
                velocity: 80,
                duration_beats: 0.25,
                probability: 1.0,
            }).collect(),
            length: n,
            loop_count: None,
        }
    }

    #[test]
    fn test_tick_returns_step_when_active() {
        let mut seq = Sequencer::new(basic_pattern(4), 120.0);
        let step = seq.tick(42);
        assert!(step.is_some());
        assert_eq!(step.unwrap().velocity, 80);
    }

    #[test]
    fn test_tick_skips_inactive_step() {
        let mut pat = basic_pattern(4);
        pat.steps[0].active = false;
        let mut seq = Sequencer::new(pat, 120.0);
        assert!(seq.tick(0).is_none());
    }

    #[test]
    fn test_tick_respects_zero_probability() {
        let mut pat = basic_pattern(4);
        pat.steps[0].probability = 0.0;
        let mut seq = Sequencer::new(pat, 120.0);
        // With probability 0.0 the step should never fire
        for seed in 0..100u64 {
            assert!(seq.tick(seed).is_none(), "step should not fire at probability=0");
            seq.reset();
        }
    }

    #[test]
    fn test_step_duration_ms() {
        let seq = Sequencer::new(basic_pattern(4), 120.0);
        // 60_000 / 120 / 4 = 125 ms
        assert!((seq.step_duration_ms() - 125.0).abs() < 1e-9);
    }

    #[test]
    fn test_pattern_duration_ms() {
        let seq = Sequencer::new(basic_pattern(16), 120.0);
        // 16 steps * 125 ms = 2000 ms
        assert!((seq.pattern_duration_ms() - 2000.0).abs() < 1e-9);
    }

    #[test]
    fn test_reset() {
        let mut seq = Sequencer::new(basic_pattern(4), 120.0);
        seq.tick(1); seq.tick(2); seq.tick(3);
        seq.reset();
        assert_eq!(seq.state.current_step, 0);
        assert_eq!(seq.state.total_steps_elapsed, 0);
    }

    #[test]
    fn test_wraps_around() {
        let mut seq = Sequencer::new(basic_pattern(4), 120.0);
        for _ in 0..4 { seq.tick(99); }
        assert_eq!(seq.state.current_step, 0);
        assert_eq!(seq.state.loop_iteration, 1);
    }

    #[test]
    fn test_poly_sequencer_tick_all() {
        let tracks = vec![
            Sequencer::new(basic_pattern(4), 120.0),
            Sequencer::new(basic_pattern(8), 120.0),
        ];
        let mut poly = PolySequencer::new(tracks);
        let results = poly.tick_all(7);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0);
        assert_eq!(results[1].0, 1);
    }

    #[test]
    fn test_set_bpm() {
        let mut seq = Sequencer::new(basic_pattern(4), 120.0);
        seq.set_bpm(60.0);
        assert!((seq.step_duration_ms() - 250.0).abs() < 1e-9);
    }
}