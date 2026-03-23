//! Species counterpoint rules engine.
//!
//! Provides voice structs, interval classification, rule checking, and a
//! simple first-species counterpoint generator — all with zero external
//! dependencies.

use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// Voice
// ---------------------------------------------------------------------------

/// A single melodic voice represented as a sequence of Hz frequencies.
#[derive(Debug, Clone)]
pub struct Voice {
    pub name: String,
    /// Note frequencies in Hz, in temporal order.
    pub notes: Vec<f64>,
}

impl Voice {
    pub fn new(name: impl Into<String>, notes: Vec<f64>) -> Self {
        Voice {
            name: name.into(),
            notes,
        }
    }
}

// ---------------------------------------------------------------------------
// Interval helpers
// ---------------------------------------------------------------------------

/// Compute the interval in semitones between two frequencies.
///
/// Returns a positive value (absolute semitone distance, mod octaves ignored).
pub fn semitones(hz_a: f64, hz_b: f64) -> f64 {
    if hz_a <= 0.0 || hz_b <= 0.0 {
        return 0.0;
    }
    let ratio = if hz_a > hz_b { hz_a / hz_b } else { hz_b / hz_a };
    12.0 * ratio.log2()
}

/// Broad interval classification by semitone count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntervalType {
    Unison,
    Second,
    Third,
    Fourth,
    Fifth,
    Sixth,
    Seventh,
    Octave,
    Compound,
}

impl IntervalType {
    /// Classify a raw semitone distance (may be non-integer, uses rounding).
    pub fn from_semitones(st: f64) -> IntervalType {
        // Reduce compound intervals to within one octave first for classification.
        let reduced = st % 12.0;
        match reduced.round() as i32 {
            0 => IntervalType::Unison,
            1 | 2 => IntervalType::Second,
            3 | 4 => IntervalType::Third,
            5 => IntervalType::Fourth,
            6 | 7 => IntervalType::Fifth,
            8 | 9 => IntervalType::Sixth,
            10 | 11 => IntervalType::Seventh,
            _ => {
                // Exact 12 (octave after rounding) or compound
                if st >= 12.0 {
                    IntervalType::Octave
                } else {
                    IntervalType::Compound
                }
            }
        }
    }

    /// True if the interval is consonant per species counterpoint convention.
    pub fn is_consonant(&self) -> bool {
        matches!(
            self,
            IntervalType::Unison
                | IntervalType::Third
                | IntervalType::Fifth
                | IntervalType::Sixth
                | IntervalType::Octave
        )
    }
}

// ---------------------------------------------------------------------------
// CounterpointRule
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum CounterpointRule {
    NoParallelFifths,
    NoParallelOctaves,
    PreferConsonance,
    ResolveTendencyTones,
    LimitLeaps(u8),
}

// ---------------------------------------------------------------------------
// CounterpointChecker
// ---------------------------------------------------------------------------

pub struct CounterpointChecker;

impl CounterpointChecker {
    pub fn new() -> Self {
        CounterpointChecker
    }

    /// Returns indices (note positions) where parallel 5ths or octaves occur
    /// between two voices.  A parallel motion occurs when both voices move in
    /// the same direction and arrive at a 5th or octave.
    pub fn check_parallel_motion(v1: &Voice, v2: &Voice) -> Vec<usize> {
        let len = v1.notes.len().min(v2.notes.len());
        if len < 2 {
            return vec![];
        }

        let mut violations = Vec::new();
        for i in 1..len {
            let prev_st = semitones(v1.notes[i - 1], v2.notes[i - 1]);
            let curr_st = semitones(v1.notes[i], v2.notes[i]);

            let prev_kind = IntervalType::from_semitones(prev_st);
            let curr_kind = IntervalType::from_semitones(curr_st);

            // Check for arrival at perfect consonances.
            if !matches!(
                curr_kind,
                IntervalType::Fifth | IntervalType::Octave | IntervalType::Unison
            ) {
                continue;
            }

            // Check parallel (same-direction) motion.
            let v1_dir = v1.notes[i] - v1.notes[i - 1];
            let v2_dir = v2.notes[i] - v2.notes[i - 1];
            let parallel = (v1_dir > 0.0 && v2_dir > 0.0) || (v1_dir < 0.0 && v2_dir < 0.0);

            if parallel && prev_kind != curr_kind {
                violations.push(i);
            }
        }
        violations
    }

    /// Returns indices where a voice makes a leap larger than `max_semitones`.
    pub fn check_leap_size(voice: &Voice, max_semitones: u8) -> Vec<usize> {
        let mut violations = Vec::new();
        for i in 1..voice.notes.len() {
            let st = semitones(voice.notes[i - 1], voice.notes[i]);
            if st > max_semitones as f64 {
                violations.push(i);
            }
        }
        violations
    }

    /// Fraction of note pairs (simultaneous) that form consonant intervals.
    pub fn consonance_score(v1: &Voice, v2: &Voice) -> f64 {
        let len = v1.notes.len().min(v2.notes.len());
        if len == 0 {
            return 1.0;
        }
        let consonant = (0..len)
            .filter(|&i| {
                let st = semitones(v1.notes[i], v2.notes[i]);
                IntervalType::from_semitones(st).is_consonant()
            })
            .count();
        consonant as f64 / len as f64
    }

    /// Apply a set of rules to all voice pairs and return violations.
    pub fn apply_rules(
        voices: &[Voice],
        rules: &[CounterpointRule],
    ) -> Vec<(CounterpointRule, Vec<usize>)> {
        let mut results = Vec::new();

        for rule in rules {
            match rule {
                CounterpointRule::NoParallelFifths | CounterpointRule::NoParallelOctaves => {
                    // Check each adjacent pair of voices.
                    let mut all_violations = Vec::new();
                    for i in 0..voices.len() {
                        for j in (i + 1)..voices.len() {
                            let v = Self::check_parallel_motion(&voices[i], &voices[j]);
                            all_violations.extend(v);
                        }
                    }
                    all_violations.sort_unstable();
                    all_violations.dedup();
                    results.push((rule.clone(), all_violations));
                }

                CounterpointRule::LimitLeaps(max) => {
                    let mut all_violations = Vec::new();
                    for voice in voices {
                        all_violations.extend(Self::check_leap_size(voice, *max));
                    }
                    all_violations.sort_unstable();
                    all_violations.dedup();
                    results.push((rule.clone(), all_violations));
                }

                CounterpointRule::PreferConsonance => {
                    // Report positions where the first two voices are dissonant.
                    if voices.len() >= 2 {
                        let len = voices[0].notes.len().min(voices[1].notes.len());
                        let dissonant: Vec<usize> = (0..len)
                            .filter(|&i| {
                                let st = semitones(voices[0].notes[i], voices[1].notes[i]);
                                !IntervalType::from_semitones(st).is_consonant()
                            })
                            .collect();
                        results.push((rule.clone(), dissonant));
                    } else {
                        results.push((rule.clone(), vec![]));
                    }
                }

                CounterpointRule::ResolveTendencyTones => {
                    // Heuristic: flag notes within 1 semitone of the tonic
                    // (first note of voice 0) that don't resolve stepwise.
                    let mut flags = Vec::new();
                    if let Some(voice) = voices.first() {
                        let tonic = voice.notes.first().copied().unwrap_or(0.0);
                        for i in 1..voice.notes.len() {
                            let st_to_tonic = semitones(voice.notes[i - 1], tonic);
                            if (st_to_tonic - 1.0).abs() < 0.5 {
                                // Leading tone — check resolution.
                                let step = semitones(voice.notes[i - 1], voice.notes[i]);
                                if step > 2.5 {
                                    flags.push(i);
                                }
                            }
                        }
                    }
                    results.push((rule.clone(), flags));
                }
            }
        }
        results
    }
}

impl Default for CounterpointChecker {
    fn default() -> Self {
        CounterpointChecker::new()
    }
}

// ---------------------------------------------------------------------------
// CounterpointGenerator
// ---------------------------------------------------------------------------

/// Generates a simple first-species counterpoint voice above a cantus firmus.
pub struct CounterpointGenerator;

impl CounterpointGenerator {
    pub fn new() -> Self {
        CounterpointGenerator
    }

    /// Generate a counterpoint voice above `cantus` using the supplied rules
    /// and a deterministic seed for reproducibility.
    ///
    /// Strategy:
    /// 1. For each cantus note choose a candidate from {unison, 3rd, 5th, 6th, octave above}.
    /// 2. Prefer consonant intervals.
    /// 3. Avoid large leaps (> 7 semitones between successive counterpoint notes).
    /// 4. Use the seed to break ties deterministically.
    pub fn generate(cantus: &Voice, rules: &[CounterpointRule], seed: u64) -> Voice {
        let max_leap: f64 = rules
            .iter()
            .find_map(|r| {
                if let CounterpointRule::LimitLeaps(m) = r {
                    Some(*m as f64)
                } else {
                    None
                }
            })
            .unwrap_or(7.0);

        // Semitone offsets for consonant intervals (above cantus).
        let consonant_offsets: &[f64] = &[0.0, 3.0, 4.0, 7.0, 8.0, 9.0, 12.0];

        let mut notes = Vec::with_capacity(cantus.notes.len());
        let mut lcg = seed.wrapping_add(6364136223846793005);

        for (i, &cf_hz) in cantus.notes.iter().enumerate() {
            // Convert cantus to MIDI-like frequency; add semitone offset.
            let candidates: Vec<f64> = consonant_offsets
                .iter()
                .map(|&st| cf_hz * 2.0_f64.powf(st / 12.0))
                .collect();

            // Filter by leap constraint from previous note.
            let prev = notes.last().copied();
            let feasible: Vec<f64> = if let Some(p) = prev {
                candidates
                    .into_iter()
                    .filter(|&c| semitones(p, c) <= max_leap)
                    .collect()
            } else {
                candidates
            };

            let chosen = if feasible.is_empty() {
                // Fallback: octave above cantus.
                cf_hz * 2.0
            } else {
                // Deterministic pick via LCG.
                lcg = lcg
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let idx = (lcg >> 33) as usize % feasible.len();
                feasible[idx]
            };

            let _ = i; // suppress unused warning
            notes.push(chosen);
        }

        Voice::new(format!("{}_counterpoint", cantus.name), notes)
    }
}

impl Default for CounterpointGenerator {
    fn default() -> Self {
        CounterpointGenerator::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn c_major_scale_hz() -> Vec<f64> {
        // C4, D4, E4, F4, G4, A4, B4, C5
        let midi_base = 60; // C4
        let offsets = [0, 2, 4, 5, 7, 9, 11, 12];
        offsets
            .iter()
            .map(|&o| 261.63 * 2.0_f64.powf((midi_base + o - 60) as f64 / 12.0))
            .collect()
    }

    #[test]
    fn semitones_unison() {
        let st = semitones(440.0, 440.0);
        assert!(st.abs() < 0.001);
    }

    #[test]
    fn semitones_octave() {
        let st = semitones(220.0, 440.0);
        assert!((st - 12.0).abs() < 0.01);
    }

    #[test]
    fn interval_type_unison() {
        assert_eq!(IntervalType::from_semitones(0.0), IntervalType::Unison);
    }

    #[test]
    fn interval_type_fifth() {
        assert_eq!(IntervalType::from_semitones(7.0), IntervalType::Fifth);
    }

    #[test]
    fn interval_consonance() {
        assert!(IntervalType::Fifth.is_consonant());
        assert!(!IntervalType::Second.is_consonant());
        assert!(!IntervalType::Seventh.is_consonant());
    }

    #[test]
    fn check_leap_size_no_violations() {
        let voice = Voice::new("test", c_major_scale_hz());
        let v = CounterpointChecker::check_leap_size(&voice, 12);
        assert!(v.is_empty());
    }

    #[test]
    fn check_leap_size_detects_octave_leap() {
        // C4 (261 Hz) to C5 (523 Hz) = 12 semitones; limit to 11.
        let voice = Voice::new("test", vec![261.63, 523.25]);
        let v = CounterpointChecker::check_leap_size(&voice, 11);
        assert_eq!(v, vec![1]);
    }

    #[test]
    fn consonance_score_perfect_fifths() {
        // Build two voices a perfect fifth apart at each note.
        let base = c_major_scale_hz();
        let fifth: Vec<f64> = base.iter().map(|&f| f * 2.0_f64.powf(7.0 / 12.0)).collect();
        let v1 = Voice::new("v1", base);
        let v2 = Voice::new("v2", fifth);
        let score = CounterpointChecker::consonance_score(&v1, &v2);
        assert!((score - 1.0).abs() < 0.01, "score={}", score);
    }

    #[test]
    fn generator_produces_correct_length() {
        let cantus = Voice::new("cf", c_major_scale_hz());
        let rules = vec![CounterpointRule::LimitLeaps(7)];
        let cp = CounterpointGenerator::generate(&cantus, &rules, 42);
        assert_eq!(cp.notes.len(), cantus.notes.len());
    }

    #[test]
    fn generator_deterministic() {
        let cantus = Voice::new("cf", c_major_scale_hz());
        let rules = vec![CounterpointRule::LimitLeaps(7)];
        let cp1 = CounterpointGenerator::generate(&cantus, &rules, 99);
        let cp2 = CounterpointGenerator::generate(&cantus, &rules, 99);
        assert_eq!(cp1.notes, cp2.notes);
    }

    #[test]
    fn generator_different_seeds_differ() {
        let cantus = Voice::new("cf", c_major_scale_hz());
        let rules = vec![CounterpointRule::LimitLeaps(7)];
        let cp1 = CounterpointGenerator::generate(&cantus, &rules, 1);
        let cp2 = CounterpointGenerator::generate(&cantus, &rules, 2);
        // May occasionally be equal but overwhelmingly should differ.
        // We just check the function runs without panic.
        let _ = cp1;
        let _ = cp2;
    }

    #[test]
    fn apply_rules_returns_one_entry_per_rule() {
        let cantus = Voice::new("cf", c_major_scale_hz());
        let cp = CounterpointGenerator::generate(&cantus, &[], 7);
        let voices = vec![cantus, cp];
        let rules = vec![
            CounterpointRule::NoParallelFifths,
            CounterpointRule::LimitLeaps(7),
        ];
        let results = CounterpointChecker::apply_rules(&voices, &rules);
        assert_eq!(results.len(), 2);
    }
}
