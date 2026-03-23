//! Scale/Mode Mapper
//!
//! Maps attractor state values to musical pitches in a given scale and mode.

use crate::blend::AttractorState;

// ── ScaleMode ─────────────────────────────────────────────────────────────────

/// Musical scale/mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    Major,
    Minor,
    Pentatonic,
    Dorian,
    Phrygian,
    Lydian,
    WholeTone,
    Chromatic,
}

// ── MusicalScale ──────────────────────────────────────────────────────────────

/// A musical scale rooted at `root_midi` (60 = middle C) in a given mode.
#[derive(Debug, Clone)]
pub struct MusicalScale {
    /// MIDI note number of the root.
    pub root_midi: u8,
    /// Scale mode.
    pub mode: ScaleMode,
}

impl MusicalScale {
    /// Create a new `MusicalScale`.
    pub fn new(root_midi: u8, mode: ScaleMode) -> Self {
        Self { root_midi, mode }
    }

    /// Return the pitch class set — semitone intervals above the root — for this mode.
    pub fn pitch_class_set(&self) -> Vec<u8> {
        match self.mode {
            ScaleMode::Major       => vec![0, 2, 4, 5, 7, 9, 11],
            ScaleMode::Minor       => vec![0, 2, 3, 5, 7, 8, 10],
            ScaleMode::Pentatonic  => vec![0, 2, 4, 7, 9],
            ScaleMode::Dorian      => vec![0, 2, 3, 5, 7, 9, 10],
            ScaleMode::Phrygian    => vec![0, 1, 3, 5, 7, 8, 10],
            ScaleMode::Lydian      => vec![0, 2, 4, 6, 7, 9, 11],
            ScaleMode::WholeTone   => vec![0, 2, 4, 6, 8, 10],
            ScaleMode::Chromatic   => (0u8..12).collect(),
        }
    }

    /// Map a value in [-1, 1] to the nearest MIDI note in the scale, across `octaves` octaves.
    ///
    /// Value -1 maps to the root; value 1 maps to the top of the last octave.
    pub fn quantize(&self, value: f64, octaves: u8) -> u8 {
        let pcs = self.pitch_class_set();
        let octaves = octaves.max(1) as usize;
        let steps_per_octave = pcs.len();
        let total_steps = steps_per_octave * octaves;

        // Map [-1,1] to [0, total_steps-1]
        let t = ((value + 1.0) * 0.5).clamp(0.0, 1.0);
        let step_f = t * (total_steps.saturating_sub(1)) as f64;
        let step = step_f.round() as usize;
        let step = step.min(total_steps - 1);

        let octave_offset = (step / steps_per_octave) as u8;
        let pc_idx = step % steps_per_octave;
        let semitone = pcs[pc_idx];

        self.root_midi
            .saturating_add(octave_offset * 12)
            .saturating_add(semitone)
    }

    /// Return a triad (root, third, fifth) appropriate for the scale degree nearest to `value`.
    ///
    /// The triad is voiced within a single octave above the quantized root.
    pub fn chord(&self, value: f64) -> [u8; 3] {
        let pcs = self.pitch_class_set();
        let total_notes = pcs.len();
        // Find the scale degree index
        let t = ((value + 1.0) * 0.5).clamp(0.0, 1.0);
        let degree = (t * (total_notes.saturating_sub(1)) as f64).round() as usize;
        let degree = degree.min(total_notes - 1);

        let root_pc  = pcs[degree % total_notes];
        let third_pc = pcs[(degree + 2) % total_notes];
        let fifth_pc = pcs[(degree + 4) % total_notes];

        // Build MIDI notes — if pc wraps around, add an octave
        let root_midi = self.root_midi.saturating_add(root_pc);
        let third_midi = if third_pc >= root_pc {
            self.root_midi.saturating_add(third_pc)
        } else {
            self.root_midi.saturating_add(third_pc).saturating_add(12)
        };
        let fifth_midi = if fifth_pc >= root_pc {
            self.root_midi.saturating_add(fifth_pc)
        } else {
            self.root_midi.saturating_add(fifth_pc).saturating_add(12)
        };

        [root_midi, third_midi, fifth_midi]
    }
}

// ── MappedPitch ───────────────────────────────────────────────────────────────

/// The result of mapping an attractor state to a musical pitch.
#[derive(Debug, Clone)]
pub struct MappedPitch {
    /// MIDI note number (0–127).
    pub midi_note: u8,
    /// Frequency in Hz (12-TET, A4 = 440 Hz).
    pub freq_hz: f32,
    /// Scale degree (0-indexed) within the scale.
    pub scale_degree: u8,
    /// The triad chord rooted at this scale degree.
    pub chord: [u8; 3],
}

// ── ScaleMapper ───────────────────────────────────────────────────────────────

/// Maps attractor state to MIDI notes in a musical scale.
pub struct ScaleMapper {
    scale: MusicalScale,
    /// Number of octaves to span.
    pub octaves: u8,
}

impl ScaleMapper {
    /// Create a new `ScaleMapper` with the given scale and octave range.
    pub fn new(scale: MusicalScale, octaves: u8) -> Self {
        Self { scale, octaves: octaves.max(1) }
    }

    /// Map an attractor state to a `MappedPitch`.
    ///
    /// Uses the `x` component of the state (normalised to [-1, 1] via tanh) to
    /// select the scale degree.
    pub fn map_state(&self, state: &AttractorState) -> MappedPitch {
        // Normalise x to [-1, 1] with tanh (soft clamp)
        let normalised = state.x.tanh();
        let pcs = self.scale.pitch_class_set();
        let total_steps = pcs.len() * self.octaves as usize;

        let t = ((normalised + 1.0) * 0.5).clamp(0.0, 1.0);
        let step_f = t * (total_steps.saturating_sub(1)) as f64;
        let step = step_f.round() as usize;
        let step = step.min(total_steps - 1);

        let octave_offset = (step / pcs.len()) as u8;
        let pc_idx = step % pcs.len();
        let semitone = pcs[pc_idx];

        let midi_note = self.scale.root_midi
            .saturating_add(octave_offset * 12)
            .saturating_add(semitone)
            .min(127);

        let freq_hz = midi_to_freq(midi_note);
        let scale_degree = pc_idx as u8;
        let chord = self.scale.chord(normalised);

        MappedPitch { midi_note, freq_hz, scale_degree, chord }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a MIDI note number to frequency in Hz (12-TET, A4 = 440 Hz).
pub fn midi_to_freq(midi_note: u8) -> f32 {
    440.0 * 2.0f32.powf((midi_note as f32 - 69.0) / 12.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // 1. Major scale pitch class set
    #[test]
    fn test_major_pcs() {
        let scale = MusicalScale::new(60, ScaleMode::Major);
        assert_eq!(scale.pitch_class_set(), vec![0, 2, 4, 5, 7, 9, 11]);
    }

    // 2. Minor scale pitch class set
    #[test]
    fn test_minor_pcs() {
        let scale = MusicalScale::new(60, ScaleMode::Minor);
        assert_eq!(scale.pitch_class_set(), vec![0, 2, 3, 5, 7, 8, 10]);
    }

    // 3. Pentatonic scale pitch class set
    #[test]
    fn test_pentatonic_pcs() {
        let scale = MusicalScale::new(60, ScaleMode::Pentatonic);
        assert_eq!(scale.pitch_class_set(), vec![0, 2, 4, 7, 9]);
    }

    // 4. Dorian scale
    #[test]
    fn test_dorian_pcs() {
        let scale = MusicalScale::new(60, ScaleMode::Dorian);
        assert_eq!(scale.pitch_class_set(), vec![0, 2, 3, 5, 7, 9, 10]);
    }

    // 5. Phrygian scale
    #[test]
    fn test_phrygian_pcs() {
        let scale = MusicalScale::new(60, ScaleMode::Phrygian);
        assert_eq!(scale.pitch_class_set(), vec![0, 1, 3, 5, 7, 8, 10]);
    }

    // 6. Lydian scale
    #[test]
    fn test_lydian_pcs() {
        let scale = MusicalScale::new(60, ScaleMode::Lydian);
        assert_eq!(scale.pitch_class_set(), vec![0, 2, 4, 6, 7, 9, 11]);
    }

    // 7. WholeTone scale has 6 notes
    #[test]
    fn test_whole_tone_count() {
        let scale = MusicalScale::new(60, ScaleMode::WholeTone);
        assert_eq!(scale.pitch_class_set().len(), 6);
    }

    // 8. Chromatic scale has 12 notes
    #[test]
    fn test_chromatic_count() {
        let scale = MusicalScale::new(60, ScaleMode::Chromatic);
        assert_eq!(scale.pitch_class_set().len(), 12);
    }

    // 9. quantize -1.0 returns root midi
    #[test]
    fn test_quantize_minus_one() {
        let scale = MusicalScale::new(60, ScaleMode::Major);
        let note = scale.quantize(-1.0, 1);
        assert_eq!(note, 60);
    }

    // 10. quantize 1.0 returns highest note in range
    #[test]
    fn test_quantize_one() {
        let scale = MusicalScale::new(60, ScaleMode::Major);
        let note = scale.quantize(1.0, 1);
        // 7 notes in major, last semitone is 11, so 60 + 11 = 71
        assert_eq!(note, 71);
    }

    // 11. quantize 0.0 returns a note within range
    #[test]
    fn test_quantize_zero_in_range() {
        let scale = MusicalScale::new(60, ScaleMode::Major);
        let note = scale.quantize(0.0, 2);
        assert!(note >= 60 && note <= 60 + 23);
    }

    // 12. quantize across 2 octaves gives higher notes
    #[test]
    fn test_quantize_two_octaves() {
        let scale = MusicalScale::new(60, ScaleMode::Major);
        let low = scale.quantize(-1.0, 2);
        let high = scale.quantize(1.0, 2);
        assert!(high > low);
    }

    // 13. chord returns 3-element array
    #[test]
    fn test_chord_length() {
        let scale = MusicalScale::new(60, ScaleMode::Major);
        let c = scale.chord(0.0);
        assert_eq!(c.len(), 3);
    }

    // 14. chord notes are in ascending or equal order (voiced above root)
    #[test]
    fn test_chord_ascending() {
        let scale = MusicalScale::new(60, ScaleMode::Major);
        let c = scale.chord(0.0);
        assert!(c[0] <= c[1]);
        assert!(c[1] <= c[2]);
    }

    // 15. midi_to_freq: middle A (69) = 440 Hz
    #[test]
    fn test_midi_to_freq_a4() {
        let f = midi_to_freq(69);
        assert!((f - 440.0).abs() < 0.01);
    }

    // 16. midi_to_freq: middle C (60) ≈ 261.63 Hz
    #[test]
    fn test_midi_to_freq_c4() {
        let f = midi_to_freq(60);
        assert!((f - 261.63).abs() < 0.1);
    }

    // 17. ScaleMapper::map_state returns valid MIDI range
    #[test]
    fn test_map_state_midi_range() {
        let scale = MusicalScale::new(60, ScaleMode::Major);
        let mapper = ScaleMapper::new(scale, 2);
        let state = AttractorState::new(1.5, 0.0, 0.0);
        let pitch = mapper.map_state(&state);
        assert!(pitch.midi_note <= 127);
    }

    // 18. ScaleMapper freq > 0
    #[test]
    fn test_map_state_freq_positive() {
        let scale = MusicalScale::new(60, ScaleMode::Minor);
        let mapper = ScaleMapper::new(scale, 1);
        let state = AttractorState::new(0.0, 0.0, 0.0);
        let pitch = mapper.map_state(&state);
        assert!(pitch.freq_hz > 0.0);
    }

    // 19. ScaleMapper chord has 3 notes
    #[test]
    fn test_map_state_chord_length() {
        let scale = MusicalScale::new(60, ScaleMode::Major);
        let mapper = ScaleMapper::new(scale, 1);
        let state = AttractorState::new(0.5, 0.0, 0.0);
        let pitch = mapper.map_state(&state);
        assert_eq!(pitch.chord.len(), 3);
    }

    // 20. quantize with clamped value > 1.0 stays within range
    #[test]
    fn test_quantize_clamps_above_one() {
        let scale = MusicalScale::new(60, ScaleMode::Chromatic);
        let n1 = scale.quantize(1.0, 1);
        let n2 = scale.quantize(5.0, 1);
        assert_eq!(n1, n2);
    }

    // 21. Pentatonic chord respects the scale's smaller set
    #[test]
    fn test_pentatonic_chord() {
        let scale = MusicalScale::new(60, ScaleMode::Pentatonic);
        let c = scale.chord(-1.0);
        // Root at -1 → degree 0 → root=60, third=pcs[2]=4→64, fifth=pcs[4]=9→69
        assert_eq!(c[0], 60);
    }

    // 22. scale_degree is within bounds
    #[test]
    fn test_scale_degree_bounds() {
        let scale = MusicalScale::new(60, ScaleMode::Major);
        let mapper = ScaleMapper::new(scale.clone(), 1);
        for v in [-1.0f64, -0.5, 0.0, 0.5, 1.0] {
            let state = AttractorState::new(v, 0.0, 0.0);
            let pitch = mapper.map_state(&state);
            assert!((pitch.scale_degree as usize) < scale.pitch_class_set().len());
        }
    }
}
