//! Chord progression generator with functional harmony.

/// Roman numeral scale degree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RomanNumeral {
    I, II, III, IV, V, VI, VII,
}

impl RomanNumeral {
    pub fn degree(&self) -> u8 {
        match self {
            RomanNumeral::I   => 1,
            RomanNumeral::II  => 2,
            RomanNumeral::III => 3,
            RomanNumeral::IV  => 4,
            RomanNumeral::V   => 5,
            RomanNumeral::VI  => 6,
            RomanNumeral::VII => 7,
        }
    }
}

/// Chord quality.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChordQuality {
    Major,
    Minor,
    Diminished,
    Augmented,
    DominantSeventh,
    MajorSeventh,
    MinorSeventh,
}

/// A chord in functional harmony context.
#[derive(Debug, Clone)]
pub struct FunctionalChord {
    pub numeral: RomanNumeral,
    pub quality: ChordQuality,
    /// Inversion: 0 = root, 1 = first, 2 = second.
    pub inversion: u8,
}

impl FunctionalChord {
    /// Harmonic tension score (0.0 = stable, 1.0 = highly tense).
    pub fn tension_score(&self) -> f64 {
        let base: f64 = match self.numeral {
            RomanNumeral::I   => 0.0,
            RomanNumeral::IV  => 0.3,
            RomanNumeral::VI  => 0.2,
            RomanNumeral::III => 0.35,
            RomanNumeral::II  => 0.5,
            RomanNumeral::VII => 0.85,
            RomanNumeral::V   => 0.6,
        };
        let quality_add: f64 = match self.quality {
            ChordQuality::Major        => 0.0,
            ChordQuality::Minor        => 0.05,
            ChordQuality::MajorSeventh => 0.1,
            ChordQuality::MinorSeventh => 0.15,
            ChordQuality::DominantSeventh => 0.4,
            ChordQuality::Diminished   => 0.35,
            ChordQuality::Augmented    => 0.3,
        };
        (base + quality_add).min(1.0)
    }
}

/// Common-practice voice-leading rules and tension calculations.
pub struct HarmonyRules;

impl HarmonyRules {
    /// Returns chords that commonly follow `from` in tonal music.
    pub fn valid_progressions(from: &RomanNumeral) -> Vec<RomanNumeral> {
        match from {
            RomanNumeral::I   => vec![RomanNumeral::IV, RomanNumeral::V, RomanNumeral::VI, RomanNumeral::II],
            RomanNumeral::II  => vec![RomanNumeral::V, RomanNumeral::VII],
            RomanNumeral::III => vec![RomanNumeral::IV, RomanNumeral::VI],
            RomanNumeral::IV  => vec![RomanNumeral::I, RomanNumeral::V, RomanNumeral::II],
            RomanNumeral::V   => vec![RomanNumeral::I, RomanNumeral::VI],
            RomanNumeral::VI  => vec![RomanNumeral::II, RomanNumeral::IV, RomanNumeral::V],
            RomanNumeral::VII => vec![RomanNumeral::I, RomanNumeral::III],
        }
    }

    /// Strength of resolution from `from` to `to` (higher = stronger resolution).
    pub fn tension_resolution(from: &FunctionalChord, to: &FunctionalChord) -> f64 {
        let tension_dropped = from.tension_score() - to.tension_score();
        tension_dropped.max(0.0)
    }
}

/// Cadence types in tonal music.
#[derive(Debug, Clone)]
pub enum CadenceType {
    Authentic,
    Half,
    Plagal,
    Deceptive,
}

fn default_quality(numeral: &RomanNumeral) -> ChordQuality {
    match numeral {
        RomanNumeral::I   => ChordQuality::Major,
        RomanNumeral::II  => ChordQuality::Minor,
        RomanNumeral::III => ChordQuality::Minor,
        RomanNumeral::IV  => ChordQuality::Major,
        RomanNumeral::V   => ChordQuality::Major,
        RomanNumeral::VI  => ChordQuality::Minor,
        RomanNumeral::VII => ChordQuality::Diminished,
    }
}

fn make_chord(numeral: RomanNumeral) -> FunctionalChord {
    let quality = default_quality(&numeral);
    FunctionalChord { numeral, quality, inversion: 0 }
}

/// LCG used for seeded random walks.
fn lcg(seed: u64, idx: usize) -> usize {
    let x = seed.wrapping_mul(6364136223846793005).wrapping_add(idx as u64 * 1442695040888963407 + 1);
    ((x >> 33) as usize)
}

pub struct ProgressionGenerator;

impl ProgressionGenerator {
    /// Generate a chord progression of `length` chords starting from `start`.
    pub fn generate(length: usize, start: RomanNumeral, seed: u64) -> Vec<FunctionalChord> {
        let mut result = Vec::with_capacity(length);
        let mut current = start;
        for i in 0..length {
            result.push(make_chord(current.clone()));
            let nexts = HarmonyRules::valid_progressions(&current);
            if nexts.is_empty() { break; }
            let pick = lcg(seed, i) % nexts.len();
            current = nexts[pick].clone();
        }
        result
    }

    /// Return a fixed cadence pattern.
    pub fn cadence(cadence_type: CadenceType) -> Vec<FunctionalChord> {
        match cadence_type {
            CadenceType::Authentic => vec![make_chord(RomanNumeral::V), make_chord(RomanNumeral::I)],
            CadenceType::Half      => vec![make_chord(RomanNumeral::I), make_chord(RomanNumeral::V)],
            CadenceType::Plagal    => vec![make_chord(RomanNumeral::IV), make_chord(RomanNumeral::I)],
            CadenceType::Deceptive => vec![make_chord(RomanNumeral::V), make_chord(RomanNumeral::VI)],
        }
    }

    /// Convert a progression to frequencies, given a root frequency and a scale
    /// as semitone offsets from root (e.g., major scale = [0,2,4,5,7,9,11]).
    pub fn to_frequencies(
        progression: &[FunctionalChord],
        root_hz: f64,
        scale: &[u8],
    ) -> Vec<Vec<f64>> {
        progression.iter().map(|chord| {
            // Degree is 1-based
            let deg = (chord.numeral.degree() as usize).saturating_sub(1);
            let root_semitone = scale.get(deg).copied().unwrap_or(0) as f64;
            let third_semitone = scale.get((deg + 2) % scale.len()).copied().unwrap_or(4) as f64;
            let fifth_semitone = scale.get((deg + 4) % scale.len()).copied().unwrap_or(7) as f64;

            let root_freq = root_hz * 2f64.powf(root_semitone / 12.0);
            let third_freq = root_hz * 2f64.powf(third_semitone / 12.0);
            let fifth_freq = root_hz * 2f64.powf(fifth_semitone / 12.0);

            let mut freqs = vec![root_freq, third_freq, fifth_freq];

            // Add seventh if applicable
            match chord.quality {
                ChordQuality::DominantSeventh | ChordQuality::MajorSeventh | ChordQuality::MinorSeventh => {
                    let seventh_semitone = scale.get((deg + 6) % scale.len()).copied().unwrap_or(11) as f64;
                    freqs.push(root_hz * 2f64.powf(seventh_semitone / 12.0));
                }
                _ => {}
            }
            freqs
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roman_numeral_degree() {
        assert_eq!(RomanNumeral::I.degree(), 1);
        assert_eq!(RomanNumeral::V.degree(), 5);
        assert_eq!(RomanNumeral::VII.degree(), 7);
    }

    #[test]
    fn test_tension_score_ordering() {
        let i_maj = FunctionalChord { numeral: RomanNumeral::I, quality: ChordQuality::Major, inversion: 0 };
        let v7 = FunctionalChord { numeral: RomanNumeral::V, quality: ChordQuality::DominantSeventh, inversion: 0 };
        assert!(i_maj.tension_score() < v7.tension_score());
    }

    #[test]
    fn test_tension_score_clamped() {
        let vii = FunctionalChord { numeral: RomanNumeral::VII, quality: ChordQuality::Diminished, inversion: 0 };
        assert!(vii.tension_score() <= 1.0);
    }

    #[test]
    fn test_valid_progressions_v_resolves_to_i() {
        let nexts = HarmonyRules::valid_progressions(&RomanNumeral::V);
        assert!(nexts.contains(&RomanNumeral::I));
    }

    #[test]
    fn test_tension_resolution_v_to_i_positive() {
        let v7 = FunctionalChord { numeral: RomanNumeral::V, quality: ChordQuality::DominantSeventh, inversion: 0 };
        let i = FunctionalChord { numeral: RomanNumeral::I, quality: ChordQuality::Major, inversion: 0 };
        let res = HarmonyRules::tension_resolution(&v7, &i);
        assert!(res > 0.0);
    }

    #[test]
    fn test_generate_length() {
        let prog = ProgressionGenerator::generate(8, RomanNumeral::I, 42);
        assert!(prog.len() <= 8);
        assert!(!prog.is_empty());
    }

    #[test]
    fn test_generate_starts_with_given_root() {
        let prog = ProgressionGenerator::generate(4, RomanNumeral::IV, 99);
        assert_eq!(prog[0].numeral, RomanNumeral::IV);
    }

    #[test]
    fn test_cadence_authentic() {
        let cad = ProgressionGenerator::cadence(CadenceType::Authentic);
        assert_eq!(cad.len(), 2);
        assert_eq!(cad[0].numeral, RomanNumeral::V);
        assert_eq!(cad[1].numeral, RomanNumeral::I);
    }

    #[test]
    fn test_cadence_plagal() {
        let cad = ProgressionGenerator::cadence(CadenceType::Plagal);
        assert_eq!(cad[0].numeral, RomanNumeral::IV);
        assert_eq!(cad[1].numeral, RomanNumeral::I);
    }

    #[test]
    fn test_to_frequencies_triad_count() {
        let major_scale: Vec<u8> = vec![0, 2, 4, 5, 7, 9, 11];
        let prog = vec![FunctionalChord { numeral: RomanNumeral::I, quality: ChordQuality::Major, inversion: 0 }];
        let freqs = ProgressionGenerator::to_frequencies(&prog, 261.63, &major_scale);
        assert_eq!(freqs.len(), 1);
        assert_eq!(freqs[0].len(), 3); // triad
    }

    #[test]
    fn test_to_frequencies_seventh_count() {
        let major_scale: Vec<u8> = vec![0, 2, 4, 5, 7, 9, 11];
        let prog = vec![FunctionalChord { numeral: RomanNumeral::V, quality: ChordQuality::DominantSeventh, inversion: 0 }];
        let freqs = ProgressionGenerator::to_frequencies(&prog, 261.63, &major_scale);
        assert_eq!(freqs[0].len(), 4); // seventh chord
    }

    #[test]
    fn test_to_frequencies_root_is_positive() {
        let major_scale: Vec<u8> = vec![0, 2, 4, 5, 7, 9, 11];
        let prog = vec![FunctionalChord { numeral: RomanNumeral::I, quality: ChordQuality::Major, inversion: 0 }];
        let freqs = ProgressionGenerator::to_frequencies(&prog, 440.0, &major_scale);
        assert!((freqs[0][0] - 440.0).abs() < 1e-9);
    }
}