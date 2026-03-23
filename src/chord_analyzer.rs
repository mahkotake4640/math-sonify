//! # Module: chord_analyzer
//!
//! Real-time chord recognition with music theory: template matching, tension
//! analysis, and harmonic rhythm tracking.

use std::collections::VecDeque;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Number of semitones in an octave.
pub const SEMITONES_IN_OCTAVE: usize = 12;

// ── ChordQuality ──────────────────────────────────────────────────────────────

/// The quality (type) of a chord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordQuality {
    /// Major triad (0, 4, 7).
    Major,
    /// Minor triad (0, 3, 7).
    Minor,
    /// Diminished triad (0, 3, 6).
    Diminished,
    /// Augmented triad (0, 4, 8).
    Augmented,
    /// Dominant 7th (0, 4, 7, 10).
    Dominant7,
    /// Major 7th (0, 4, 7, 11).
    Major7,
    /// Minor 7th (0, 3, 7, 10).
    Minor7,
    /// Half-diminished 7th (0, 3, 6, 10).
    HalfDim7,
    /// Fully-diminished 7th (0, 3, 6, 9).
    Dim7,
    /// Suspended 2nd (0, 2, 7).
    Sus2,
    /// Suspended 4th (0, 5, 7).
    Sus4,
}

impl ChordQuality {
    /// Return the semitone intervals relative to root that define this quality.
    pub fn intervals(&self) -> Vec<u8> {
        match self {
            ChordQuality::Major      => vec![0, 4, 7],
            ChordQuality::Minor      => vec![0, 3, 7],
            ChordQuality::Diminished => vec![0, 3, 6],
            ChordQuality::Augmented  => vec![0, 4, 8],
            ChordQuality::Dominant7  => vec![0, 4, 7, 10],
            ChordQuality::Major7     => vec![0, 4, 7, 11],
            ChordQuality::Minor7     => vec![0, 3, 7, 10],
            ChordQuality::HalfDim7   => vec![0, 3, 6, 10],
            ChordQuality::Dim7       => vec![0, 3, 6, 9],
            ChordQuality::Sus2       => vec![0, 2, 7],
            ChordQuality::Sus4       => vec![0, 5, 7],
        }
    }
}

// ── ChordTemplate ─────────────────────────────────────────────────────────────

/// A chord defined by a root pitch class and quality.
#[derive(Clone, Debug)]
pub struct ChordTemplate {
    /// Root pitch class (0 = C, 1 = C#, …, 11 = B).
    pub root: u8,
    /// Quality of the chord.
    pub quality: ChordQuality,
    /// Semitone intervals relative to root.
    pub intervals: Vec<u8>,
}

impl ChordTemplate {
    /// Create a new template, computing absolute pitch classes from root + intervals.
    pub fn new(root: u8, quality: ChordQuality) -> Self {
        let intervals = quality.intervals();
        Self { root, quality, intervals }
    }

    /// Absolute pitch classes of the chord tones.
    pub fn pitch_classes(&self) -> Vec<u8> {
        self.intervals
            .iter()
            .map(|&i| (self.root + i) % SEMITONES_IN_OCTAVE as u8)
            .collect()
    }
}

// ── build_chord_templates ─────────────────────────────────────────────────────

/// Build all 12 roots × all qualities → 132 templates.
pub fn build_chord_templates() -> Vec<ChordTemplate> {
    let qualities = [
        ChordQuality::Major,
        ChordQuality::Minor,
        ChordQuality::Diminished,
        ChordQuality::Augmented,
        ChordQuality::Dominant7,
        ChordQuality::Major7,
        ChordQuality::Minor7,
        ChordQuality::HalfDim7,
        ChordQuality::Dim7,
        ChordQuality::Sus2,
        ChordQuality::Sus4,
    ];
    let mut templates = Vec::with_capacity(12 * qualities.len());
    for root in 0u8..12 {
        for quality in &qualities {
            templates.push(ChordTemplate::new(root, quality.clone()));
        }
    }
    templates
}

// ── normalize_pitch_class ─────────────────────────────────────────────────────

/// Reduce a MIDI note number to a pitch class (0–11).
pub fn normalize_pitch_class(midi: u8) -> u8 {
    midi % SEMITONES_IN_OCTAVE as u8
}

// ── match_chord ───────────────────────────────────────────────────────────────

/// Score each template against an observed set of pitch classes.
///
/// Score = `coverage - false_positives` where:
/// - `coverage` = fraction of template tones present in `pitch_classes`.
/// - `false_positives` = fraction of `pitch_classes` notes not in template.
///
/// Returns a list of `(score, template)` pairs for templates with score > 0.
pub fn match_chord(pitch_classes: &[u8]) -> Vec<(f64, ChordTemplate)> {
    if pitch_classes.is_empty() {
        return vec![];
    }
    let templates = build_chord_templates();
    let mut results = Vec::new();
    let observed: std::collections::HashSet<u8> = pitch_classes
        .iter()
        .map(|&m| normalize_pitch_class(m))
        .collect();

    for tmpl in templates {
        let template_pcs = tmpl.pitch_classes();
        let template_set: std::collections::HashSet<u8> =
            template_pcs.iter().copied().collect();

        let hits = template_set.intersection(&observed).count() as f64;
        let coverage = hits / template_set.len() as f64;
        let false_pos = (observed.len() as f64 - hits) / observed.len().max(1) as f64;
        let score = coverage - false_pos;
        if score > 0.0 {
            results.push((score, tmpl));
        }
    }
    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    results
}

// ── TensionAnalyzer ───────────────────────────────────────────────────────────

/// Computes tension and resolution tendency between chords.
pub struct TensionAnalyzer;

impl TensionAnalyzer {
    /// Return a tension score for a chord (higher = more tense).
    ///
    /// 7th chords and diminished chords score higher than triads.
    pub fn tension_score(&self, chord: &ChordTemplate) -> f64 {
        match chord.quality {
            ChordQuality::Major | ChordQuality::Sus2 | ChordQuality::Sus4 => 0.1,
            ChordQuality::Minor => 0.2,
            ChordQuality::Augmented => 0.5,
            ChordQuality::Diminished => 0.6,
            ChordQuality::Dominant7 => 0.8,
            ChordQuality::Major7 => 0.4,
            ChordQuality::Minor7 => 0.45,
            ChordQuality::HalfDim7 => 0.7,
            ChordQuality::Dim7 => 0.9,
        }
    }

    /// Estimate how strongly `from` resolves to `to` (0.0 = no tendency, 1.0 = strong).
    ///
    /// Uses a simple root-motion heuristic: a perfect-fifth descent (7 semitones)
    /// from `from` to `to` signals strong resolution.
    pub fn resolution_tendency(&self, from: &ChordTemplate, to: &ChordTemplate) -> f64 {
        let interval = (from.root as i16 - to.root as i16).rem_euclid(12) as u8;
        // Dominant → tonic resolution: root descends a perfect 5th (7 semitones up = 5 down).
        if interval == 7 {
            return 1.0;
        }
        // Leading-tone resolution: semitone descent
        if interval == 1 {
            return 0.8;
        }
        // Tritone substitution: tritone
        if interval == 6 {
            return 0.6;
        }
        0.1
    }
}

// ── ChordAnalyzer ─────────────────────────────────────────────────────────────

/// Stateful chord recogniser that tracks history for harmonic rhythm analysis.
pub struct ChordAnalyzer {
    /// All templates (built once at construction).
    pub templates: Vec<ChordTemplate>,
    /// Recent chord history for harmonic rhythm.
    pub history: VecDeque<ChordTemplate>,
}

impl ChordAnalyzer {
    /// Create a new analyser.
    pub fn new() -> Self {
        Self {
            templates: build_chord_templates(),
            history: VecDeque::with_capacity(32),
        }
    }

    /// Recognise the best-matching chord from a set of MIDI note numbers.
    ///
    /// Returns `None` if no chord scores above zero.
    pub fn analyze(&mut self, midi_notes: &[u8]) -> Option<ChordTemplate> {
        let mut matches = match_chord(midi_notes);
        let best = matches.drain(..).next()?.1;
        // Append to history (cap at 32).
        if self.history.len() >= 32 {
            self.history.pop_front();
        }
        self.history.push_back(best.clone());
        Some(best)
    }

    /// Estimate harmonic rhythm: chord changes per chord in recent history.
    ///
    /// Returns the fraction of adjacent pairs in history that differ.
    pub fn harmonic_rhythm(&self) -> f64 {
        if self.history.len() < 2 {
            return 0.0;
        }
        let changes = self
            .history
            .iter()
            .zip(self.history.iter().skip(1))
            .filter(|(a, b)| a.root != b.root || a.quality != b.quality)
            .count();
        changes as f64 / (self.history.len() - 1) as f64
    }
}

impl Default for ChordAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_major_recognized() {
        // C=60, E=64, G=67
        let notes = [60u8, 64, 67];
        let matches = match_chord(&notes);
        assert!(!matches.is_empty());
        let (_, best) = &matches[0];
        assert_eq!(best.root, 0);
        assert_eq!(best.quality, ChordQuality::Major);
    }

    #[test]
    fn tension_ordering() {
        let ta = TensionAnalyzer;
        let maj = ChordTemplate::new(0, ChordQuality::Major);
        let dim7 = ChordTemplate::new(0, ChordQuality::Dim7);
        assert!(ta.tension_score(&dim7) > ta.tension_score(&maj));
    }

    #[test]
    fn resolution_g7_to_c() {
        let ta = TensionAnalyzer;
        let g7 = ChordTemplate::new(7, ChordQuality::Dominant7);
        let c  = ChordTemplate::new(0, ChordQuality::Major);
        assert_eq!(ta.resolution_tendency(&g7, &c), 1.0);
    }

    #[test]
    fn harmonic_rhythm_all_same() {
        let mut ca = ChordAnalyzer::new();
        for _ in 0..4 {
            ca.analyze(&[60, 64, 67]);
        }
        assert_eq!(ca.harmonic_rhythm(), 0.0);
    }
}
