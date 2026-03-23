//! Alternative tuning systems and just intonation.
//!
//! Provides frequency computation for MIDI notes under several historical and
//! theoretical tuning systems, precomputed frequency tables, and just-intonation
//! interval ratio helpers.

// ── TuningSystem ──────────────────────────────────────────────────────────────

/// A tuning system used to compute frequencies for MIDI notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuningSystem {
    /// Standard 12-TET: A4 = concert_a, each semitone = 2^(1/12).
    EqualTemperament,
    /// 5-limit just intonation built around C major.
    JustIntonation,
    /// Pythagorean tuning: stack perfect fifths (3/2) and reduce by octaves.
    Pythagorean,
    /// Quarter-comma meantone: flattened fifth ≈ 1.49535.
    QuarterCommaMeantone,
    /// Werckmeister III: specific cents offsets from equal temperament.
    Werckmeister3,
}

// ── Frequency calculation ─────────────────────────────────────────────────────

/// Compute the frequency (Hz) of a MIDI note in the given tuning system.
///
/// `concert_a` is the frequency of A4 (MIDI note 69); typically 440.0 Hz.
pub fn frequency(system: TuningSystem, midi_note: u8, concert_a: f64) -> f64 {
    match system {
        TuningSystem::EqualTemperament => et_frequency(midi_note, concert_a),
        TuningSystem::JustIntonation => ji_frequency(midi_note, concert_a),
        TuningSystem::Pythagorean => pythagorean_frequency(midi_note, concert_a),
        TuningSystem::QuarterCommaMeantone => meantone_frequency(midi_note, concert_a),
        TuningSystem::Werckmeister3 => werckmeister3_frequency(midi_note, concert_a),
    }
}

// ── Equal temperament ─────────────────────────────────────────────────────────

fn et_frequency(midi_note: u8, concert_a: f64) -> f64 {
    concert_a * 2_f64.powf((midi_note as f64 - 69.0) / 12.0)
}

// ── Just intonation (5-limit) ─────────────────────────────────────────────────

/// 5-limit just ratios for the 12 pitch classes relative to C.
///
/// Indices: C=0, C#=1, D=2, Eb=3, E=4, F=5, F#=6, G=7, Ab=8, A=9, Bb=10, B=11.
const JUST_RATIOS: [f64; 12] = [
    1.0,          // C   1/1
    16.0 / 15.0,  // C#  16/15
    9.0 / 8.0,    // D   9/8
    6.0 / 5.0,    // Eb  6/5
    5.0 / 4.0,    // E   5/4
    4.0 / 3.0,    // F   4/3
    45.0 / 32.0,  // F#  45/32
    3.0 / 2.0,    // G   3/2
    8.0 / 5.0,    // Ab  8/5
    5.0 / 3.0,    // A   5/3
    9.0 / 5.0,    // Bb  9/5
    15.0 / 8.0,   // B   15/8
];

fn ji_frequency(midi_note: u8, concert_a: f64) -> f64 {
    // MIDI 60 = C4.  A4 = MIDI 69 (pitch class 9, octave 4).
    let pc = midi_note % 12;         // pitch class 0..11
    let octave = (midi_note / 12) as i32 - 5; // octave relative to octave 5 (C5)

    // Frequency of C in the same octave as A4, based on concert_a.
    // A4 = concert_a; A is pitch class 9 in our table.
    // C4 = concert_a / (JUST_RATIOS[9] * 2^(octave_of_A4 - octave_of_C4))
    // A4 is octave 4 (midi 69 / 12 = 5 → octave index 5, relative = 5-5=0).
    // C4: midi 60 / 12 = 5, octave index 5, relative = 5-5 = 0.
    // So C4 freq = concert_a / JUST_RATIOS[9].
    let c4_freq = concert_a / JUST_RATIOS[9];

    // Target octave (index 5 = octave 4 in MIDI convention).
    let midi_octave_idx = midi_note / 12; // 5 for octave 4
    let octave_from_c4 = midi_octave_idx as i32 - 5;

    c4_freq * JUST_RATIOS[pc as usize] * 2_f64.powi(octave_from_c4)
}

// ── Pythagorean ───────────────────────────────────────────────────────────────

/// Pythagorean ratios for the 12 pitch classes relative to C.
///
/// Built by stacking perfect fifths (3/2) and reducing to one octave.
/// The sequence of fifths from C: C, G, D, A, E, B, F#, C#, G#/Ab, Eb, Bb, F.
fn pythagorean_ratio(pc: u8) -> f64 {
    // Fifth index for each pitch class (number of fifths up from C, mod 12).
    // C=0, G=1, D=2, A=3, E=4, B=5, F#=6, C#=7, G#=8, D#=9, A#=10, F=-1→11.
    const FIFTH_IDX: [i32; 12] = [0, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10, 5];
    let n = FIFTH_IDX[pc as usize];
    let ratio = (3_f64 / 2.0).powi(n);
    // Reduce to [1, 2) by dividing by appropriate power of 2.
    let octaves = ratio.log2().floor() as i32;
    ratio / 2_f64.powi(octaves)
}

fn pythagorean_frequency(midi_note: u8, concert_a: f64) -> f64 {
    let pc = midi_note % 12;
    let midi_octave_idx = midi_note / 12;
    let octave_from_c4 = midi_octave_idx as i32 - 5;

    // A4 has pitch class 9; ratio relative to C.
    let a4_ratio = pythagorean_ratio(9);
    let c4_freq = concert_a / a4_ratio;

    c4_freq * pythagorean_ratio(pc) * 2_f64.powi(octave_from_c4)
}

// ── Quarter-comma meantone ────────────────────────────────────────────────────

/// The meantone fifth: 2^(1/4) * 5^(1/4).
const MEANTONE_FIFTH: f64 = 1.495348781_f64; // (5.0_f64).sqrt().sqrt() * (2.0_f64).sqrt().sqrt()

fn meantone_ratio(pc: u8) -> f64 {
    // Same fifth-index mapping as Pythagorean.
    const FIFTH_IDX: [i32; 12] = [0, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10, 5];
    let n = FIFTH_IDX[pc as usize];
    let ratio = MEANTONE_FIFTH.powi(n);
    let octaves = ratio.log2().floor() as i32;
    ratio / 2_f64.powi(octaves)
}

fn meantone_frequency(midi_note: u8, concert_a: f64) -> f64 {
    let pc = midi_note % 12;
    let midi_octave_idx = midi_note / 12;
    let octave_from_c4 = midi_octave_idx as i32 - 5;

    let a4_ratio = meantone_ratio(9);
    let c4_freq = concert_a / a4_ratio;

    c4_freq * meantone_ratio(pc) * 2_f64.powi(octave_from_c4)
}

// ── Werckmeister III ──────────────────────────────────────────────────────────

/// Cents offsets from equal temperament for the 12 pitch classes (C=0..B=11).
const WERCKMEISTER3_CENTS: [f64; 12] = [
    0.0, 90.0, 192.0, 294.0, 390.0, 498.0,
    588.0, 696.0, 792.0, 888.0, 996.0, 1092.0,
];

fn werckmeister3_frequency(midi_note: u8, concert_a: f64) -> f64 {
    // Start from C4 as reference.
    let pc = midi_note % 12;
    let midi_octave_idx = midi_note / 12;
    let octave_from_c4 = midi_octave_idx as i32 - 5;

    // A4 cents offset from C4 in Werckmeister III.
    let a4_cents = WERCKMEISTER3_CENTS[9]; // 888 cents from C4
    let pc_cents = WERCKMEISTER3_CENTS[pc as usize];

    // C4 frequency.
    let c4_freq = concert_a / 2_f64.powf(a4_cents / 1200.0);

    c4_freq * 2_f64.powf(pc_cents / 1200.0) * 2_f64.powi(octave_from_c4)
}

// ── TuningTable ───────────────────────────────────────────────────────────────

/// Precomputed frequency table for all 128 MIDI notes.
#[derive(Debug, Clone)]
pub struct TuningTable {
    system: TuningSystem,
    concert_a: f64,
    frequencies: [f64; 128],
}

impl TuningTable {
    /// Build a tuning table for the given system and concert A pitch.
    pub fn build(system: TuningSystem, concert_a: f64) -> Self {
        let mut frequencies = [0.0_f64; 128];
        for note in 0u8..=127 {
            frequencies[note as usize] = frequency(system, note, concert_a);
        }
        Self { system, concert_a, frequencies }
    }

    /// Get the frequency of a MIDI note.
    pub fn get(&self, midi_note: u8) -> f64 {
        self.frequencies[midi_note as usize]
    }

    /// Deviation from equal temperament in cents.
    ///
    /// Positive = sharper than ET, negative = flatter.
    pub fn cents_deviation_from_et(&self, midi_note: u8) -> f64 {
        let this_freq = self.get(midi_note);
        let et_freq = et_frequency(midi_note, self.concert_a);
        1200.0 * (this_freq / et_freq).log2()
    }
}

// ── IntervalRatio ─────────────────────────────────────────────────────────────

/// A just intonation interval ratio (numerator/denominator).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntervalRatio {
    pub numerator: u32,
    pub denominator: u32,
}

impl IntervalRatio {
    /// Create a new interval ratio.
    pub fn new(numerator: u32, denominator: u32) -> Self {
        Self { numerator, denominator }
    }

    /// Convert ratio to cents: 1200 * log2(num / denom).
    pub fn to_cents(&self) -> f64 {
        1200.0 * (self.numerator as f64 / self.denominator as f64).log2()
    }

    /// The interval as a frequency multiplier.
    pub fn to_ratio(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    // ── Common intervals ──────────────────────────────────────────────────────

    /// Perfect unison: 1/1 (0 cents).
    pub fn unison() -> Self { Self::new(1, 1) }
    /// Minor second: 16/15 (~112 cents).
    pub fn minor_second() -> Self { Self::new(16, 15) }
    /// Major second: 9/8 (~204 cents).
    pub fn major_second() -> Self { Self::new(9, 8) }
    /// Minor third: 6/5 (~316 cents).
    pub fn minor_third() -> Self { Self::new(6, 5) }
    /// Major third: 5/4 (~386 cents).
    pub fn major_third() -> Self { Self::new(5, 4) }
    /// Perfect fourth: 4/3 (~498 cents).
    pub fn perfect_fourth() -> Self { Self::new(4, 3) }
    /// Tritone: 45/32 (~590 cents).
    pub fn tritone() -> Self { Self::new(45, 32) }
    /// Perfect fifth: 3/2 (~702 cents).
    pub fn perfect_fifth() -> Self { Self::new(3, 2) }
    /// Minor sixth: 8/5 (~814 cents).
    pub fn minor_sixth() -> Self { Self::new(8, 5) }
    /// Major sixth: 5/3 (~884 cents).
    pub fn major_sixth() -> Self { Self::new(5, 3) }
    /// Minor seventh: 9/5 (~1018 cents).
    pub fn minor_seventh() -> Self { Self::new(9, 5) }
    /// Major seventh: 15/8 (~1088 cents).
    pub fn major_seventh() -> Self { Self::new(15, 8) }
    /// Perfect octave: 2/1 (1200 cents).
    pub fn octave() -> Self { Self::new(2, 1) }
}

impl std::fmt::Display for IntervalRatio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{} ({:.1} cents)", self.numerator, self.denominator, self.to_cents())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const CONCERT_A: f64 = 440.0;
    const EPSILON: f64 = 1e-6;

    #[test]
    fn et_a4_is_440() {
        let f = frequency(TuningSystem::EqualTemperament, 69, CONCERT_A);
        assert!((f - 440.0).abs() < EPSILON, "ET A4 = {}", f);
    }

    #[test]
    fn et_a3_is_220() {
        let f = frequency(TuningSystem::EqualTemperament, 57, CONCERT_A);
        assert!((f - 220.0).abs() < EPSILON, "ET A3 = {}", f);
    }

    #[test]
    fn et_c4_correct() {
        // C4 = A4 * 2^(-9/12) = 440 * 2^(-0.75) ≈ 261.626
        let f = frequency(TuningSystem::EqualTemperament, 60, CONCERT_A);
        let expected = 440.0 * 2_f64.powf(-9.0 / 12.0);
        assert!((f - expected).abs() < EPSILON);
    }

    #[test]
    fn tuning_table_matches_direct() {
        let table = TuningTable::build(TuningSystem::EqualTemperament, CONCERT_A);
        for note in 0u8..=127 {
            let direct = frequency(TuningSystem::EqualTemperament, note, CONCERT_A);
            let from_table = table.get(note);
            assert!((direct - from_table).abs() < EPSILON,
                "note {}: direct={} table={}", note, direct, from_table);
        }
    }

    #[test]
    fn et_cents_deviation_is_zero() {
        let table = TuningTable::build(TuningSystem::EqualTemperament, CONCERT_A);
        for note in 0u8..=127 {
            let dev = table.cents_deviation_from_et(note);
            assert!(dev.abs() < 1e-9, "ET self-deviation note {}: {}", note, dev);
        }
    }

    #[test]
    fn pythagorean_fifth_near_702_cents() {
        // Pythagorean G above C: should be exactly 3/2 → 701.955 cents.
        let c = frequency(TuningSystem::Pythagorean, 60, CONCERT_A);
        let g = frequency(TuningSystem::Pythagorean, 67, CONCERT_A);
        let cents = 1200.0 * (g / c).log2();
        // Pythagorean fifth = 701.955 cents.
        assert!((cents - 701.955).abs() < 0.01, "Pythagorean fifth = {} cents", cents);
    }

    #[test]
    fn just_intonation_a4_correct() {
        let f = frequency(TuningSystem::JustIntonation, 69, CONCERT_A);
        // A4 in JI = concert_a by definition.
        assert!((f - CONCERT_A).abs() < EPSILON, "JI A4 = {}", f);
    }

    #[test]
    fn just_major_third_ratio() {
        // E4 / C4 should be close to 5/4 = 1.25 in JI.
        let c4 = frequency(TuningSystem::JustIntonation, 60, CONCERT_A);
        let e4 = frequency(TuningSystem::JustIntonation, 64, CONCERT_A);
        let ratio = e4 / c4;
        assert!((ratio - 5.0 / 4.0).abs() < 1e-9, "JI major third ratio = {}", ratio);
    }

    #[test]
    fn interval_ratio_perfect_fifth() {
        let fifth = IntervalRatio::perfect_fifth();
        assert_eq!(fifth.numerator, 3);
        assert_eq!(fifth.denominator, 2);
        let cents = fifth.to_cents();
        assert!((cents - 701.955).abs() < 0.001, "fifth cents = {}", cents);
    }

    #[test]
    fn interval_ratio_octave_is_1200_cents() {
        let oct = IntervalRatio::octave();
        assert!((oct.to_cents() - 1200.0).abs() < 1e-9);
    }

    #[test]
    fn interval_ratio_unison_is_zero_cents() {
        assert!((IntervalRatio::unison().to_cents()).abs() < 1e-9);
    }

    #[test]
    fn werckmeister3_a4_correct() {
        let f = frequency(TuningSystem::Werckmeister3, 69, CONCERT_A);
        // A4 cents offset from C4 in W3 is 888 cents.
        // C4_freq = 440 / 2^(888/1200). A4 = C4 * 2^(888/1200) = 440.
        assert!((f - CONCERT_A).abs() < EPSILON, "W3 A4 = {}", f);
    }
}
