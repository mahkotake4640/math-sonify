//! Euclidean rhythm generator using Bjorklund's algorithm.
//!
//! Euclidean rhythms distribute `pulses` beats as evenly as possible across
//! `steps` time slots.  They appear in many world-music traditions and are
//! commonly used in electronic music.
//!
//! # Reference
//! Toussaint, G. (2005). "The Euclidean algorithm generates traditional musical
//! rhythms." *Proceedings of BRIDGES*.

// ── MidiEvent (minimal, standalone) ──────────────────────────────────────────

/// A simple MIDI event used for Euclidean rhythm output.
#[derive(Debug, Clone, PartialEq)]
pub enum MidiEvent {
    /// Note On: `(tick, note, velocity)`.
    NoteOn { tick: u32, note: u8, velocity: u8 },
    /// Note Off: `(tick, note)`.
    NoteOff { tick: u32, note: u8 },
}

// ── Bjorklund's algorithm ─────────────────────────────────────────────────────

/// Generate an Euclidean rhythm using Bjorklund's algorithm.
///
/// Returns a bit pattern of length `steps` with exactly `pulses` `true`
/// values distributed as evenly as possible.
///
/// # Panics
/// Panics if `pulses > steps`.
pub fn bjorklund(steps: usize, pulses: usize) -> Vec<bool> {
    assert!(pulses <= steps, "pulses ({}) must be <= steps ({})", pulses, steps);

    if pulses == 0 {
        return vec![false; steps];
    }
    if pulses == steps {
        return vec![true; steps];
    }

    // Represent the pattern as groups of sequences.
    // Each group is a Vec<bool>.
    let mut groups: Vec<Vec<bool>> = (0..pulses)
        .map(|_| vec![true])
        .collect();
    let mut remainders: Vec<Vec<bool>> = (0..(steps - pulses))
        .map(|_| vec![false])
        .collect();

    loop {
        if remainders.len() <= 1 {
            break;
        }
        let pairs = remainders.len().min(groups.len());
        let mut new_groups: Vec<Vec<bool>> = Vec::with_capacity(pairs);
        for i in 0..pairs {
            let mut combined = groups[i].clone();
            combined.extend_from_slice(&remainders[i]);
            new_groups.push(combined);
        }

        let old_groups = groups;
        let old_remainders = remainders;

        if old_groups.len() > pairs {
            // Some groups become the new remainders.
            remainders = old_groups[pairs..].to_vec();
            groups = new_groups;
        } else {
            // Some remainders become the new remainders.
            remainders = old_remainders[pairs..].to_vec();
            groups = new_groups;
        }
    }

    let mut pattern: Vec<bool> = Vec::with_capacity(steps);
    for g in &groups {
        pattern.extend_from_slice(g);
    }
    for r in &remainders {
        pattern.extend_from_slice(r);
    }

    pattern
}

// ── EuclideanRhythm ───────────────────────────────────────────────────────────

/// A stateful Euclidean rhythm player.
#[derive(Debug, Clone)]
pub struct EuclideanRhythm {
    /// The bit pattern (true = pulse, false = rest).
    pub pattern: Vec<bool>,
    /// Current step index (wraps around).
    pub current_step: usize,
    /// Tempo in beats per minute.
    pub bpm: f64,
    /// Number of pattern steps per beat.
    pub steps_per_beat: usize,
}

impl EuclideanRhythm {
    /// Create a new rhythm from an explicit pattern.
    pub fn new(pattern: Vec<bool>, bpm: f64, steps_per_beat: usize) -> Self {
        Self { pattern, current_step: 0, bpm, steps_per_beat }
    }

    /// Create a rhythm using Bjorklund's algorithm.
    pub fn from_bjorklund(steps: usize, pulses: usize, bpm: f64, steps_per_beat: usize) -> Self {
        Self::new(bjorklund(steps, pulses), bpm, steps_per_beat)
    }

    /// Advance by one step and return `true` if this step has a pulse.
    pub fn tick(&mut self) -> bool {
        if self.pattern.is_empty() {
            return false;
        }
        let hit = self.pattern[self.current_step];
        self.current_step = (self.current_step + 1) % self.pattern.len();
        hit
    }

    /// Emit NoteOn/NoteOff MIDI event pairs for each pulse in the pattern.
    ///
    /// Events are spaced `ticks_per_step` ticks apart, calculated from BPM,
    /// a standard 480 PPQN resolution, and `steps_per_beat`.
    pub fn to_midi_events(&self, note: u8, velocity: u8, duration_ticks: u32) -> Vec<MidiEvent> {
        const PPQN: u32 = 480;
        let ticks_per_beat = PPQN;
        let ticks_per_step = if self.steps_per_beat > 0 {
            ticks_per_beat / self.steps_per_beat as u32
        } else {
            ticks_per_beat
        };

        let mut events = Vec::new();
        for (i, &pulse) in self.pattern.iter().enumerate() {
            if pulse {
                let on_tick = i as u32 * ticks_per_step;
                let off_tick = on_tick + duration_ticks;
                events.push(MidiEvent::NoteOn { tick: on_tick, note, velocity });
                events.push(MidiEvent::NoteOff { tick: off_tick, note });
            }
        }
        // Sort by tick for proper ordering.
        events.sort_by_key(|e| match e {
            MidiEvent::NoteOn { tick, .. } => *tick,
            MidiEvent::NoteOff { tick, .. } => *tick,
        });
        events
    }

    // ── Classic rhythms ───────────────────────────────────────────────────────

    /// Son clave: E(3,8), the standard 3-2 clave pattern.
    pub fn clave_son() -> Self {
        Self::from_bjorklund(8, 3, 120.0, 2)
    }

    /// Bossa nova: E(3,16).
    pub fn bossa_nova() -> Self {
        Self::from_bjorklund(16, 3, 120.0, 4)
    }

    /// Tresillo: E(3,8) rotated by 1 step.
    pub fn tresillo() -> Self {
        let pattern = rotate(&bjorklund(8, 3), 1);
        Self::new(pattern, 120.0, 2)
    }

    /// Shiko: E(4,8).
    pub fn shiko() -> Self {
        Self::from_bjorklund(8, 4, 120.0, 2)
    }
}

// ── Helper functions ──────────────────────────────────────────────────────────

/// Rotate a pattern left by `amount` steps.
pub fn rotate(pattern: &[bool], amount: usize) -> Vec<bool> {
    if pattern.is_empty() {
        return Vec::new();
    }
    let amount = amount % pattern.len();
    let mut result = Vec::with_capacity(pattern.len());
    result.extend_from_slice(&pattern[amount..]);
    result.extend_from_slice(&pattern[..amount]);
    result
}

/// Render a pattern as ASCII art: `X` for pulse, `.` for rest.
pub fn to_ascii(pattern: &[bool]) -> String {
    pattern.iter().map(|&b| if b { 'X' } else { '.' }).collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e3_8_correct() {
        // E(3,8) = [1,0,0,1,0,0,1,0]
        let pattern = bjorklund(8, 3);
        assert_eq!(pattern.len(), 8);
        assert_eq!(pattern.iter().filter(|&&b| b).count(), 3);
        // The standard Euclidean E(3,8) pattern.
        assert_eq!(pattern, vec![true, false, false, true, false, false, true, false]);
    }

    #[test]
    fn e5_8_correct() {
        // E(5,8) = [1,0,1,1,0,1,1,0]
        let pattern = bjorklund(8, 5);
        assert_eq!(pattern.len(), 8);
        assert_eq!(pattern.iter().filter(|&&b| b).count(), 5);
        assert_eq!(pattern, vec![true, false, true, true, false, true, true, false]);
    }

    #[test]
    fn pulse_count_always_correct() {
        for steps in 1..=16 {
            for pulses in 0..=steps {
                let pattern = bjorklund(steps, pulses);
                assert_eq!(pattern.len(), steps, "wrong length for E({},{})", pulses, steps);
                assert_eq!(
                    pattern.iter().filter(|&&b| b).count(),
                    pulses,
                    "wrong pulse count for E({},{})",
                    pulses,
                    steps
                );
            }
        }
    }

    #[test]
    fn rotate_works() {
        let p = vec![true, false, false, true, false, false, true, false];
        let r = rotate(&p, 2);
        assert_eq!(r[0], false);
        assert_eq!(r[1], true);
        // Length preserved.
        assert_eq!(r.len(), p.len());
    }

    #[test]
    fn to_ascii_correct() {
        let pattern = bjorklund(8, 3);
        assert_eq!(to_ascii(&pattern), "X..X..X.");
    }

    #[test]
    fn tick_wraps_around() {
        let mut r = EuclideanRhythm::clave_son();
        let hits: Vec<bool> = (0..16).map(|_| r.tick()).collect();
        // First 8 ticks should match the pattern exactly, second 8 the same.
        assert_eq!(hits[0], hits[8]);
        assert_eq!(hits[1], hits[9]);
    }

    #[test]
    fn midi_events_count() {
        let rhythm = EuclideanRhythm::clave_son();
        let events = rhythm.to_midi_events(60, 100, 50);
        // 3 pulses → 3 NoteOn + 3 NoteOff = 6 events.
        assert_eq!(events.len(), 6);
    }

    #[test]
    fn classic_rhythms_compile() {
        let _son = EuclideanRhythm::clave_son();
        let _bossa = EuclideanRhythm::bossa_nova();
        let _tres = EuclideanRhythm::tresillo();
        let _shiko = EuclideanRhythm::shiko();
    }
}
