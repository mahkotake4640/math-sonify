//! End-to-end data → music sonification pipeline.
//!
//! Converts sequences of [`DataPoint`] values into [`Note`] events via
//! normalisation, pitch/velocity/duration mapping, and additive sine synthesis.

use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single labelled data observation.
#[derive(Debug, Clone)]
pub struct DataPoint {
    pub value: f64,
    pub timestamp: u64,
    pub label: String,
}

/// A musical note produced by the pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// Fundamental frequency in Hz.
    pub pitch_hz: f64,
    /// MIDI-style velocity (0–127).
    pub velocity: u8,
    /// Duration in beats.
    pub duration_beats: f64,
    /// Original timestamp from the data point.
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// NormalizationMethod
// ---------------------------------------------------------------------------

/// How raw data values are normalised to the [0, 1] range.
#[derive(Debug, Clone)]
pub enum NormalizationMethod {
    /// (v - min) / (max - min)
    MinMax,
    /// (v - mean) / std, then mapped to [0, 1] via sigmoid
    ZScore,
    /// Rank-based percentile
    Percentile(u8),
    /// 1 / (1 + exp(-v))
    Sigmoid,
}

// ---------------------------------------------------------------------------
// MappingConfig
// ---------------------------------------------------------------------------

/// Configuration for mapping normalised values onto musical parameters.
#[derive(Debug, Clone)]
pub struct MappingConfig {
    /// (low_hz, high_hz) pitch range.
    pub pitch_range_hz: (f64, f64),
    /// (min_vel, max_vel) velocity range.
    pub velocity_range: (u8, u8),
    /// (min_beats, max_beats) duration range.
    pub duration_range_beats: (f64, f64),
    /// Whether to use rhythm modulation.
    pub use_rhythm: bool,
    /// Beats per minute.
    pub bpm: f64,
}

impl Default for MappingConfig {
    fn default() -> Self {
        Self {
            pitch_range_hz: (110.0, 880.0),
            velocity_range: (40, 120),
            duration_range_beats: (0.25, 2.0),
            use_rhythm: false,
            bpm: 120.0,
        }
    }
}

// ---------------------------------------------------------------------------
// SonificationPipeline
// ---------------------------------------------------------------------------

/// Transforms raw data into playable musical notes and audio samples.
pub struct SonificationPipeline;

impl SonificationPipeline {
    pub fn new() -> Self {
        Self
    }

    /// Normalise data values to the range [0, 1].
    pub fn normalize(&self, data: &[DataPoint], method: &NormalizationMethod) -> Vec<f64> {
        if data.is_empty() {
            return vec![];
        }
        let values: Vec<f64> = data.iter().map(|d| d.value).collect();

        match method {
            NormalizationMethod::MinMax => {
                let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let range = max - min;
                if range == 0.0 {
                    return vec![0.5; data.len()];
                }
                values.iter().map(|v| (v - min) / range).collect()
            }
            NormalizationMethod::ZScore => {
                let n = values.len() as f64;
                let mean = values.iter().sum::<f64>() / n;
                let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
                let std = variance.sqrt().max(1e-9);
                values
                    .iter()
                    .map(|v| sigmoid((v - mean) / std))
                    .collect()
            }
            NormalizationMethod::Percentile(pct) => {
                let p = (*pct as f64).clamp(0.0, 100.0) / 100.0;
                let mut sorted = values.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let threshold_idx = ((sorted.len() as f64 - 1.0) * p) as usize;
                let threshold = sorted[threshold_idx];
                let min = sorted[0];
                let max = *sorted.last().unwrap();
                let range = max - min;
                if range == 0.0 {
                    return vec![0.5; data.len()];
                }
                values
                    .iter()
                    .map(|v| (v.min(threshold) - min) / range)
                    .collect()
            }
            NormalizationMethod::Sigmoid => values.iter().map(|v| sigmoid(*v)).collect(),
        }
    }

    /// Map a normalised value [0, 1] to a pitch in Hz using log-frequency interpolation.
    pub fn map_to_pitch(&self, normalized: f64, config: &MappingConfig) -> f64 {
        let (lo, hi) = config.pitch_range_hz;
        let lo_log = lo.max(1.0).ln();
        let hi_log = hi.max(lo + 1.0).ln();
        let n = normalized.clamp(0.0, 1.0);
        (lo_log + n * (hi_log - lo_log)).exp()
    }

    /// Map a normalised value [0, 1] to a MIDI velocity.
    pub fn map_to_velocity(&self, normalized: f64, config: &MappingConfig) -> u8 {
        let (lo, hi) = config.velocity_range;
        let n = normalized.clamp(0.0, 1.0);
        (lo as f64 + n * (hi as f64 - lo as f64)).round() as u8
    }

    /// Map a normalised value [0, 1] to a duration in beats.
    pub fn map_to_duration(&self, normalized: f64, config: &MappingConfig) -> f64 {
        let (lo, hi) = config.duration_range_beats;
        let n = normalized.clamp(0.0, 1.0);
        lo + n * (hi - lo)
    }

    /// Full pipeline: data → normalise → map → Vec<Note>.
    pub fn sonify(
        &self,
        data: &[DataPoint],
        config: &MappingConfig,
        method: &NormalizationMethod,
    ) -> Vec<Note> {
        let normalized = self.normalize(data, method);
        data.iter()
            .zip(normalized.iter())
            .map(|(dp, &n)| Note {
                pitch_hz: self.map_to_pitch(n, config),
                velocity: self.map_to_velocity(n, config),
                duration_beats: self.map_to_duration(n, config),
                timestamp: dp.timestamp,
            })
            .collect()
    }

    /// Render a sequence of notes to audio samples via additive sine synthesis.
    ///
    /// Each note is synthesised as a single sine wave at `pitch_hz` with an
    /// amplitude proportional to `velocity`.  Notes are placed sequentially
    /// based on their duration and the configured BPM.
    pub fn render_audio(&self, notes: &[Note], sample_rate: u32, bpm: f64) -> Vec<f64> {
        if notes.is_empty() {
            return vec![];
        }
        let sr = sample_rate as f64;
        let beat_duration_s = 60.0 / bpm.max(1.0);

        // Compute total samples needed
        let total_beats: f64 = notes.iter().map(|n| n.duration_beats).sum();
        let total_samples = (total_beats * beat_duration_s * sr).ceil() as usize + 1;

        let mut buffer = vec![0.0f64; total_samples];
        let mut cursor_samples = 0usize;

        for note in notes {
            let amplitude = note.velocity as f64 / 127.0;
            let duration_s = note.duration_beats * beat_duration_s;
            let note_samples = (duration_s * sr).round() as usize;
            let freq = note.pitch_hz;

            for i in 0..note_samples {
                let t = i as f64 / sr;
                // Simple sine with linear fade-out envelope
                let env = 1.0 - (i as f64 / note_samples.max(1) as f64);
                let sample = amplitude * env * (2.0 * PI * freq * t).sin();
                let idx = cursor_samples + i;
                if idx < buffer.len() {
                    buffer[idx] += sample;
                }
            }
            cursor_samples += note_samples;
        }

        // Normalise to [-1, 1]
        let peak = buffer.iter().cloned().map(f64::abs).fold(0.0f64, f64::max);
        if peak > 0.0 {
            buffer.iter_mut().for_each(|s| *s /= peak);
        }
        buffer
    }
}

impl Default for SonificationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> Vec<DataPoint> {
        vec![
            DataPoint { value: 1.0, timestamp: 0, label: "a".into() },
            DataPoint { value: 3.0, timestamp: 1, label: "b".into() },
            DataPoint { value: 2.0, timestamp: 2, label: "c".into() },
            DataPoint { value: 5.0, timestamp: 3, label: "d".into() },
            DataPoint { value: 4.0, timestamp: 4, label: "e".into() },
        ]
    }

    fn pipeline() -> SonificationPipeline {
        SonificationPipeline::new()
    }

    // --- normalisation ---

    #[test]
    fn normalize_minmax_range() {
        let p = pipeline();
        let data = sample_data();
        let n = p.normalize(&data, &NormalizationMethod::MinMax);
        assert!((n[0] - 0.0).abs() < 1e-9); // min = 1.0
        assert!((n[3] - 1.0).abs() < 1e-9); // max = 5.0
        assert!(n.iter().all(|&v| v >= 0.0 && v <= 1.0));
    }

    #[test]
    fn normalize_zscore_range() {
        let p = pipeline();
        let data = sample_data();
        let n = p.normalize(&data, &NormalizationMethod::ZScore);
        assert!(n.iter().all(|&v| v >= 0.0 && v <= 1.0));
    }

    #[test]
    fn normalize_sigmoid_range() {
        let p = pipeline();
        let data = sample_data();
        let n = p.normalize(&data, &NormalizationMethod::Sigmoid);
        assert!(n.iter().all(|&v| v > 0.0 && v < 1.0));
    }

    #[test]
    fn normalize_percentile_range() {
        let p = pipeline();
        let data = sample_data();
        let n = p.normalize(&data, &NormalizationMethod::Percentile(90));
        assert!(n.iter().all(|&v| v >= 0.0 && v <= 1.0));
    }

    #[test]
    fn normalize_empty_returns_empty() {
        let p = pipeline();
        assert!(p.normalize(&[], &NormalizationMethod::MinMax).is_empty());
    }

    // --- mapping ---

    #[test]
    fn map_to_pitch_bounds() {
        let p = pipeline();
        let cfg = MappingConfig::default();
        let low = p.map_to_pitch(0.0, &cfg);
        let high = p.map_to_pitch(1.0, &cfg);
        assert!((low - cfg.pitch_range_hz.0).abs() < 1e-6);
        assert!((high - cfg.pitch_range_hz.1).abs() < 1e-6);
    }

    #[test]
    fn map_to_velocity_bounds() {
        let p = pipeline();
        let cfg = MappingConfig::default();
        assert_eq!(p.map_to_velocity(0.0, &cfg), cfg.velocity_range.0);
        assert_eq!(p.map_to_velocity(1.0, &cfg), cfg.velocity_range.1);
    }

    #[test]
    fn map_to_duration_bounds() {
        let p = pipeline();
        let cfg = MappingConfig::default();
        let short = p.map_to_duration(0.0, &cfg);
        let long = p.map_to_duration(1.0, &cfg);
        assert!((short - cfg.duration_range_beats.0).abs() < 1e-9);
        assert!((long - cfg.duration_range_beats.1).abs() < 1e-9);
    }

    // --- sonify ---

    #[test]
    fn sonify_returns_correct_count() {
        let p = pipeline();
        let data = sample_data();
        let cfg = MappingConfig::default();
        let notes = p.sonify(&data, &cfg, &NormalizationMethod::MinMax);
        assert_eq!(notes.len(), data.len());
    }

    #[test]
    fn sonify_pitch_monotone_with_sorted_data() {
        let p = pipeline();
        let data = vec![
            DataPoint { value: 0.0, timestamp: 0, label: "a".into() },
            DataPoint { value: 0.5, timestamp: 1, label: "b".into() },
            DataPoint { value: 1.0, timestamp: 2, label: "c".into() },
        ];
        let cfg = MappingConfig::default();
        let notes = p.sonify(&data, &cfg, &NormalizationMethod::MinMax);
        assert!(notes[0].pitch_hz <= notes[1].pitch_hz);
        assert!(notes[1].pitch_hz <= notes[2].pitch_hz);
    }

    #[test]
    fn render_audio_nonzero() {
        let p = pipeline();
        let cfg = MappingConfig::default();
        let data = sample_data();
        let notes = p.sonify(&data, &cfg, &NormalizationMethod::MinMax);
        let audio = p.render_audio(&notes, 44100, 120.0);
        assert!(!audio.is_empty());
        let peak = audio.iter().cloned().map(f64::abs).fold(0.0f64, f64::max);
        assert!(peak > 0.0);
        // Normalised so peak <= 1.0
        assert!(peak <= 1.0 + 1e-9);
    }

    #[test]
    fn render_audio_empty_notes() {
        let p = pipeline();
        let audio = p.render_audio(&[], 44100, 120.0);
        assert!(audio.is_empty());
    }

    #[test]
    fn pitch_log_scale_midpoint() {
        // The log-scale midpoint should be the geometric mean of lo and hi.
        let p = pipeline();
        let cfg = MappingConfig {
            pitch_range_hz: (100.0, 1000.0),
            ..MappingConfig::default()
        };
        let mid = p.map_to_pitch(0.5, &cfg);
        let geom_mean = (100.0f64 * 1000.0).sqrt();
        assert!((mid - geom_mean).abs() < 1.0, "mid={mid} geom_mean={geom_mean}");
    }
}
