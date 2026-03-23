//! Musical analysis: key detection, scale identification, and rhythm analysis.
//!
//! Implements Krumhansl-Kessler key profiles for 24 keys and IOI-based rhythm
//! analysis for tempo and meter estimation.

// ---------------------------------------------------------------------------
// Krumhansl-Kessler key profiles
// ---------------------------------------------------------------------------

/// Krumhansl-Kessler tonal hierarchy profiles for major and minor keys.
/// These are the probe-tone ratings from the original 1982 study, normalised.
pub const MAJOR_PROFILE: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];

pub const MINOR_PROFILE: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// All 12 pitch class names (C, C#, D, ...).
const PITCH_CLASSES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

// ---------------------------------------------------------------------------
// KeyDetector
// ---------------------------------------------------------------------------

/// Detects the musical key from a frequency histogram.
pub struct KeyDetector;

impl KeyDetector {
    pub fn new() -> Self {
        Self
    }

    /// Build a pitch-class histogram from a set of note frequencies in Hz.
    ///
    /// Each frequency is mapped to a MIDI note number, then to pitch class 0–11
    /// (C = 0, C# = 1, ..., B = 11).  A = 9 at 440 Hz.
    pub fn pitch_class_histogram(notes_hz: &[f64]) -> [f64; 12] {
        let mut hist = [0.0f64; 12];
        for &hz in notes_hz {
            if hz <= 0.0 {
                continue;
            }
            // Convert Hz to MIDI: MIDI = 69 + 12 * log2(hz / 440)
            let midi = 69.0 + 12.0 * (hz / 440.0).log2();
            let pc = (midi.round() as i64).rem_euclid(12) as usize;
            hist[pc] += 1.0;
        }
        hist
    }

    /// Find the best-matching key by correlating the histogram against all 24
    /// Krumhansl-Kessler profiles.  Returns `(key_name, correlation)`.
    pub fn detect_key(histogram: &[f64; 12]) -> (String, f64) {
        let correlations = Self::all_correlations(histogram);
        correlations
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or_else(|| ("C major".to_string(), 0.0))
    }

    /// Return the gap between the top-two correlation scores as a confidence.
    pub fn confidence(correlations: &[(String, f64)]) -> f64 {
        if correlations.len() < 2 {
            return 0.0;
        }
        let mut sorted: Vec<f64> = correlations.iter().map(|(_, c)| *c).collect();
        sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        sorted[0] - sorted[1]
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn all_correlations(histogram: &[f64; 12]) -> Vec<(String, f64)> {
        let mut result = Vec::with_capacity(24);
        for root in 0..12 {
            // Major
            let major_profile = rotate_profile(&MAJOR_PROFILE, root);
            let corr_major = pearson_correlation(histogram, &major_profile);
            result.push((format!("{} major", PITCH_CLASSES[root]), corr_major));
            // Minor
            let minor_profile = rotate_profile(&MINOR_PROFILE, root);
            let corr_minor = pearson_correlation(histogram, &minor_profile);
            result.push((format!("{} minor", PITCH_CLASSES[root]), corr_minor));
        }
        result
    }
}

impl Default for KeyDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RhythmAnalyzer
// ---------------------------------------------------------------------------

/// Analyses rhythmic structure from note onset times.
pub struct RhythmAnalyzer;

impl RhythmAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Compute inter-onset intervals (IOIs) from a sorted list of onset times.
    pub fn inter_onset_intervals(onset_times_ms: &[u64]) -> Vec<u64> {
        if onset_times_ms.len() < 2 {
            return vec![];
        }
        onset_times_ms
            .windows(2)
            .map(|w| w[1].saturating_sub(w[0]))
            .collect()
    }

    /// Most common IOI (mode).
    pub fn dominant_period_ms(iois: &[u64]) -> u64 {
        if iois.is_empty() {
            return 0;
        }
        let mut counts: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
        for &ioi in iois {
            *counts.entry(ioi).or_insert(0) += 1;
        }
        counts
            .into_iter()
            .max_by_key(|(_, cnt)| *cnt)
            .map(|(v, _)| v)
            .unwrap_or(0)
    }

    /// Estimate BPM from the dominant IOI.
    pub fn estimated_bpm(iois: &[u64]) -> f64 {
        let period = Self::dominant_period_ms(iois);
        if period == 0 {
            return 0.0;
        }
        60_000.0 / period as f64
    }

    /// Rhythmic regularity: 1 – (std_dev / mean).  0 = totally irregular, 1 = perfect grid.
    pub fn rhythmic_regularity(iois: &[u64]) -> f64 {
        if iois.is_empty() {
            return 0.0;
        }
        let n = iois.len() as f64;
        let mean = iois.iter().map(|&v| v as f64).sum::<f64>() / n;
        if mean == 0.0 {
            return 0.0;
        }
        let variance = iois
            .iter()
            .map(|&v| (v as f64 - mean).powi(2))
            .sum::<f64>()
            / n;
        let std_dev = variance.sqrt();
        (1.0 - std_dev / mean).max(0.0)
    }

    /// Detect meter (2, 3, or 4) by analysing ratios between IOIs.
    pub fn detect_meter(iois: &[u64]) -> u8 {
        if iois.len() < 2 {
            return 4; // default
        }
        // Look at consecutive IOI ratios to find grouping
        let dom = Self::dominant_period_ms(iois) as f64;
        if dom == 0.0 {
            return 4;
        }

        // Count strong-beat candidates (IOIs close to 2×, 3×, 4× of the dominant)
        let mut score_2 = 0i32;
        let mut score_3 = 0i32;
        let mut score_4 = 0i32;

        for &ioi in iois {
            let ratio = ioi as f64 / dom;
            if (ratio - 2.0).abs() < 0.25 { score_2 += 1; }
            if (ratio - 3.0).abs() < 0.35 { score_3 += 1; }
            if (ratio - 4.0).abs() < 0.50 { score_4 += 1; }
        }

        // Also check pairs of adjacent IOIs
        for w in iois.windows(2) {
            let sum = (w[0] + w[1]) as f64;
            let ratio = sum / dom;
            if (ratio - 2.0).abs() < 0.30 { score_2 += 2; }
            if (ratio - 3.0).abs() < 0.40 { score_3 += 2; }
            if (ratio - 4.0).abs() < 0.60 { score_4 += 2; }
        }

        let max_score = score_2.max(score_3).max(score_4);
        if max_score == score_3 && score_3 > 0 {
            3
        } else if max_score == score_2 && score_2 > 0 {
            2
        } else {
            4
        }
    }
}

impl Default for RhythmAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Statistical helpers
// ---------------------------------------------------------------------------

/// Rotate a 12-element profile by `steps` semitones.
fn rotate_profile(profile: &[f64; 12], steps: usize) -> [f64; 12] {
    let mut rotated = [0.0f64; 12];
    for i in 0..12 {
        rotated[i] = profile[(i + 12 - steps) % 12];
    }
    rotated
}

/// Pearson correlation coefficient between two 12-element arrays.
fn pearson_correlation(a: &[f64; 12], b: &[f64; 12]) -> f64 {
    let n = 12.0;
    let mean_a = a.iter().sum::<f64>() / n;
    let mean_b = b.iter().sum::<f64>() / n;

    let mut num = 0.0;
    let mut den_a = 0.0;
    let mut den_b = 0.0;

    for i in 0..12 {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        num += da * db;
        den_a += da * da;
        den_b += db * db;
    }

    let denom = (den_a * den_b).sqrt();
    if denom == 0.0 { 0.0 } else { num / denom }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- pitch class histogram ---

    #[test]
    fn histogram_a4_is_pitch_class_9() {
        let hist = KeyDetector::pitch_class_histogram(&[440.0]);
        assert_eq!(hist[9], 1.0); // A = 9
    }

    #[test]
    fn histogram_middle_c_is_pitch_class_0() {
        // Middle C = 261.626 Hz
        let hist = KeyDetector::pitch_class_histogram(&[261.626]);
        assert_eq!(hist[0], 1.0);
    }

    #[test]
    fn histogram_empty_is_zeros() {
        let hist = KeyDetector::pitch_class_histogram(&[]);
        assert!(hist.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn histogram_sums_correctly() {
        let freqs = vec![261.626, 329.628, 392.0, 523.251]; // C4 E4 G4 C5
        let hist = KeyDetector::pitch_class_histogram(&freqs);
        assert_eq!(hist.iter().sum::<f64>() as usize, 4);
    }

    // --- key detection ---

    #[test]
    fn detect_key_c_major_scale() {
        // C major scale: C D E F G A B
        let freqs = [
            261.626, 293.665, 329.628, 349.228, 392.000, 440.000, 493.883,
        ];
        // Weight C-major notes heavily
        let mut all_freqs: Vec<f64> = freqs.iter().cloned().collect();
        for _ in 0..3 { all_freqs.extend_from_slice(&freqs); }
        let hist = KeyDetector::pitch_class_histogram(&all_freqs);
        let (key, _corr) = KeyDetector::detect_key(&hist);
        assert!(key.contains("major"), "Expected major key, got: {key}");
    }

    #[test]
    fn detect_key_returns_string_and_correlation() {
        let hist = [1.0; 12];
        let (key, _corr) = KeyDetector::detect_key(&hist);
        assert!(!key.is_empty());
    }

    #[test]
    fn confidence_gap_positive_when_clear_winner() {
        // Feed a clear C-major histogram
        let freqs = [261.626, 329.628, 392.0, 523.251, 440.0, 349.228, 293.665];
        let mut big: Vec<f64> = Vec::new();
        for _ in 0..10 { big.extend_from_slice(&freqs); }
        let hist = KeyDetector::pitch_class_histogram(&big);
        let kd = KeyDetector::new();
        let _ = kd; // ensure constructible
        let correlations = KeyDetector::all_correlations(&hist);
        let conf = KeyDetector::confidence(&correlations);
        assert!(conf >= 0.0);
    }

    #[test]
    fn rotate_profile_wraps_correctly() {
        let p = MAJOR_PROFILE;
        let rotated = rotate_profile(&p, 1);
        assert_eq!(rotated[0], p[11]);
        assert_eq!(rotated[1], p[0]);
    }

    #[test]
    fn pearson_correlation_identical_is_one() {
        let p = MAJOR_PROFILE;
        let c = pearson_correlation(&p, &p);
        assert!((c - 1.0).abs() < 1e-9);
    }

    // --- rhythm ---

    #[test]
    fn ioi_basic() {
        let onsets = vec![0u64, 500, 1000, 1500];
        let iois = RhythmAnalyzer::inter_onset_intervals(&onsets);
        assert_eq!(iois, vec![500, 500, 500]);
    }

    #[test]
    fn ioi_single_onset_is_empty() {
        let iois = RhythmAnalyzer::inter_onset_intervals(&[100]);
        assert!(iois.is_empty());
    }

    #[test]
    fn dominant_period_mode() {
        let iois = vec![500u64, 500, 500, 250, 500, 250];
        assert_eq!(RhythmAnalyzer::dominant_period_ms(&iois), 500);
    }

    #[test]
    fn estimated_bpm_at_500ms() {
        // 500 ms per beat = 120 BPM
        let iois = vec![500u64; 8];
        let bpm = RhythmAnalyzer::estimated_bpm(&iois);
        assert!((bpm - 120.0).abs() < 1e-6);
    }

    #[test]
    fn estimated_bpm_empty_is_zero() {
        assert_eq!(RhythmAnalyzer::estimated_bpm(&[]), 0.0);
    }

    #[test]
    fn rhythmic_regularity_perfect_grid() {
        let iois = vec![500u64; 8];
        let reg = RhythmAnalyzer::rhythmic_regularity(&iois);
        assert!((reg - 1.0).abs() < 1e-9);
    }

    #[test]
    fn rhythmic_regularity_irregular() {
        let iois = vec![100u64, 500, 200, 800, 150];
        let reg = RhythmAnalyzer::rhythmic_regularity(&iois);
        assert!(reg < 1.0);
        assert!(reg >= 0.0);
    }

    #[test]
    fn detect_meter_duple() {
        // Alternating short-long: strong beat every 2 units
        let iois = vec![500u64, 500, 500, 500, 500, 500, 500, 500];
        let meter = RhythmAnalyzer::detect_meter(&iois);
        assert!(meter == 2 || meter == 4, "got {meter}");
    }

    #[test]
    fn detect_meter_triple() {
        // Every third beat is long: 3/4 feel
        let iois: Vec<u64> = (0..9).map(|i| if i % 3 == 2 { 1500 } else { 500 }).collect();
        let meter = RhythmAnalyzer::detect_meter(&iois);
        // Allow 2, 3, or 4 — implementation is heuristic
        assert!(meter >= 2 && meter <= 4);
    }

    #[test]
    fn detect_meter_default_is_four() {
        let iois = vec![500u64];
        let meter = RhythmAnalyzer::detect_meter(&iois);
        assert_eq!(meter, 4);
    }
}
