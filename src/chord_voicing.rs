//! Chord voicing with inversions and voice leading.
//!
//! Provides chord construction, inversions, open/drop-2 voicings, voice
//! leading cost calculation, and smooth chord progression generation.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Interval
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Interval {
    Unison,
    Minor2,
    Major2,
    Minor3,
    Major3,
    Perfect4,
    Tritone,
    Perfect5,
    Minor6,
    Major6,
    Minor7,
    Major7,
    Octave,
}

impl Interval {
    pub fn semitones(&self) -> u8 {
        match self {
            Interval::Unison    => 0,
            Interval::Minor2    => 1,
            Interval::Major2    => 2,
            Interval::Minor3    => 3,
            Interval::Major3    => 4,
            Interval::Perfect4  => 5,
            Interval::Tritone   => 6,
            Interval::Perfect5  => 7,
            Interval::Minor6    => 8,
            Interval::Major6    => 9,
            Interval::Minor7    => 10,
            Interval::Major7    => 11,
            Interval::Octave    => 12,
        }
    }
}

// ---------------------------------------------------------------------------
// ChordQuality
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChordQuality {
    Major,
    Minor,
    Diminished,
    Augmented,
    Dom7,
    Maj7,
    Min7,
    Dim7,
    HalfDim7,
    Sus2,
    Sus4,
}

// ---------------------------------------------------------------------------
// chord_intervals
// ---------------------------------------------------------------------------

/// Return the semitone intervals above the root for the given quality.
pub fn chord_intervals(quality: &ChordQuality) -> Vec<u8> {
    match quality {
        ChordQuality::Major      => vec![0, 4, 7],
        ChordQuality::Minor      => vec![0, 3, 7],
        ChordQuality::Diminished => vec![0, 3, 6],
        ChordQuality::Augmented  => vec![0, 4, 8],
        ChordQuality::Dom7       => vec![0, 4, 7, 10],
        ChordQuality::Maj7       => vec![0, 4, 7, 11],
        ChordQuality::Min7       => vec![0, 3, 7, 10],
        ChordQuality::Dim7       => vec![0, 3, 6, 9],
        ChordQuality::HalfDim7   => vec![0, 3, 6, 10],
        ChordQuality::Sus2       => vec![0, 2, 7],
        ChordQuality::Sus4       => vec![0, 5, 7],
    }
}

// ---------------------------------------------------------------------------
// Chord
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Chord {
    /// Root note as MIDI pitch number.
    pub root: u8,
    pub quality: ChordQuality,
    pub octave: u8,
    pub notes: Vec<u8>,
}

impl Chord {
    /// Build a chord from a MIDI root note.
    pub fn build(root_midi: u8, quality: ChordQuality) -> Self {
        let intervals = chord_intervals(&quality);
        let octave = root_midi / 12;
        let notes: Vec<u8> = intervals
            .iter()
            .map(|&i| root_midi.saturating_add(i))
            .collect();
        Self {
            root: root_midi,
            quality,
            octave,
            notes,
        }
    }

    /// Return the chord in inversion `inversion` (0 = root position).
    pub fn invert(&self, inversion: u8) -> Self {
        if self.notes.is_empty() || inversion == 0 {
            return self.clone();
        }
        let inv = (inversion as usize) % self.notes.len();
        let mut notes = self.notes.clone();
        for _ in 0..inv {
            // Move bottom note up by an octave
            let bottom = notes.remove(0);
            notes.push(bottom.saturating_add(12));
        }
        Self {
            root: self.root,
            quality: self.quality.clone(),
            octave: self.octave,
            notes,
        }
    }

    /// Open voicing: alternate notes between low and high register.
    pub fn voicing_open(&self) -> Vec<u8> {
        let mut low = Vec::new();
        let mut high = Vec::new();
        for (i, &note) in self.notes.iter().enumerate() {
            if i % 2 == 0 {
                low.push(note);
            } else {
                high.push(note.saturating_add(12));
            }
        }
        let mut result = low;
        result.extend(high);
        result.sort_unstable();
        result
    }

    /// Drop-2 voicing: second voice from top dropped an octave.
    pub fn voicing_drop2(&self) -> Vec<u8> {
        if self.notes.len() < 2 {
            return self.notes.clone();
        }
        let mut notes = self.notes.clone();
        notes.sort_unstable();
        let len = notes.len();
        let second_from_top = len - 2;
        if notes[second_from_top] >= 12 {
            notes[second_from_top] -= 12;
        }
        notes.sort_unstable();
        notes
    }
}

// ---------------------------------------------------------------------------
// Voice leading
// ---------------------------------------------------------------------------

/// Calculate the voice leading cost (sum of absolute pitch distances) between two chords.
pub fn voice_leading_cost(a: &Chord, b: &Chord) -> f64 {
    let len = a.notes.len().max(b.notes.len());
    let mut a_notes = a.notes.clone();
    let mut b_notes = b.notes.clone();

    // Pad shorter chord by repeating last note
    while a_notes.len() < len {
        let last = *a_notes.last().unwrap_or(&0);
        a_notes.push(last);
    }
    while b_notes.len() < len {
        let last = *b_notes.last().unwrap_or(&0);
        b_notes.push(last);
    }

    a_notes
        .iter()
        .zip(b_notes.iter())
        .map(|(&x, &y)| (x as i32 - y as i32).unsigned_abs() as f64)
        .sum()
}

/// Find the inversion of `to_quality` rooted at `to_root` that minimizes voice leading cost from `from`.
pub fn find_best_inversion(from: &Chord, to_root: u8, to_quality: ChordQuality) -> Chord {
    let base = Chord::build(to_root, to_quality);
    let num_notes = base.notes.len();
    let mut best_chord = base.invert(0);
    let mut best_cost = voice_leading_cost(from, &best_chord);

    for inv in 1..num_notes as u8 {
        let candidate = base.invert(inv);
        let cost = voice_leading_cost(from, &candidate);
        if cost < best_cost {
            best_cost = cost;
            best_chord = candidate;
        }
    }
    best_chord
}

// ---------------------------------------------------------------------------
// ChordProgression
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ChordProgression {
    pub chords: Vec<Chord>,
    pub tempo_bpm: u32,
}

impl ChordProgression {
    /// Build a progression with smooth voice leading by greedily choosing inversions.
    pub fn smooth_voice_leading(
        roots: &[u8],
        qualities: &[ChordQuality],
        tempo_bpm: u32,
    ) -> Self {
        assert_eq!(roots.len(), qualities.len(), "roots and qualities must have same length");

        let mut chords: Vec<Chord> = Vec::new();

        for (i, (&root, quality)) in roots.iter().zip(qualities.iter()).enumerate() {
            if i == 0 {
                chords.push(Chord::build(root, quality.clone()));
            } else {
                let prev = &chords[i - 1];
                let best = find_best_inversion(prev, root, quality.clone());
                chords.push(best);
            }
        }

        Self { chords, tempo_bpm }
    }
}

// ---------------------------------------------------------------------------
// MIDI event generation
// ---------------------------------------------------------------------------

/// Convert a chord progression to MIDI events: (time_in_ticks, note_vec).
/// Uses 480 ticks per beat.
pub fn to_midi_events(prog: &ChordProgression, beats_per_chord: u32) -> Vec<(u32, Vec<u8>)> {
    const TICKS_PER_BEAT: u32 = 480;
    let ticks_per_chord = TICKS_PER_BEAT * beats_per_chord;
    prog.chords
        .iter()
        .enumerate()
        .map(|(i, chord)| (i as u32 * ticks_per_chord, chord.notes.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_major_chord_has_three_notes() {
        let chord = Chord::build(60, ChordQuality::Major); // C4
        assert_eq!(chord.notes.len(), 3);
        // C major: C E G = 60 64 67
        assert_eq!(chord.notes, vec![60, 64, 67]);
    }

    #[test]
    fn test_first_inversion_shifts_root_up() {
        let chord = Chord::build(60, ChordQuality::Major); // C E G
        let inv1 = chord.invert(1); // E G C(+12)
        // Root (60) should have moved up by 12 to 72
        assert!(inv1.notes.contains(&72));
        assert!(!inv1.notes.contains(&60));
    }

    #[test]
    fn test_voice_leading_cost_same_chord() {
        let chord = Chord::build(60, ChordQuality::Major);
        let cost = voice_leading_cost(&chord, &chord);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_find_best_inversion_returns_valid_chord() {
        let from = Chord::build(60, ChordQuality::Major); // C major
        let best = find_best_inversion(&from, 64, ChordQuality::Minor); // E minor
        assert!(!best.notes.is_empty());
        // All notes should be valid MIDI (0-127)
        for &n in &best.notes {
            assert!(n <= 127);
        }
    }

    #[test]
    fn test_smooth_progression_same_length_as_input() {
        let roots = vec![60u8, 65, 67, 72];
        let qualities = vec![
            ChordQuality::Major,
            ChordQuality::Major,
            ChordQuality::Major,
            ChordQuality::Major,
        ];
        let prog = ChordProgression::smooth_voice_leading(&roots, &qualities, 120);
        assert_eq!(prog.chords.len(), roots.len());
    }

    #[test]
    fn test_dom7_chord_has_four_notes() {
        let chord = Chord::build(60, ChordQuality::Dom7);
        assert_eq!(chord.notes.len(), 4);
    }

    #[test]
    fn test_open_voicing_non_empty() {
        let chord = Chord::build(60, ChordQuality::Major);
        let open = chord.voicing_open();
        assert_eq!(open.len(), 3);
    }

    #[test]
    fn test_drop2_voicing_non_empty() {
        let chord = Chord::build(60, ChordQuality::Maj7);
        let drop2 = chord.voicing_drop2();
        assert_eq!(drop2.len(), 4);
    }

    #[test]
    fn test_midi_events_count() {
        let roots = vec![60u8, 65, 67];
        let qualities = vec![ChordQuality::Major, ChordQuality::Minor, ChordQuality::Major];
        let prog = ChordProgression::smooth_voice_leading(&roots, &qualities, 120);
        let events = to_midi_events(&prog, 4);
        assert_eq!(events.len(), 3);
        // First event at tick 0
        assert_eq!(events[0].0, 0);
    }
}
