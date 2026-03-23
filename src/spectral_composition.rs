//! # Spectral Composition Mode
//!
//! Instead of mapping ODE state variables to audio parameters, directly
//! synthesise audio whose *spectrum IS* the attractor's power spectrum.
//!
//! ## Method
//!
//! 1. Collect a window of attractor trajectory samples (x, y, z over T steps).
//! 2. Compute the FFT of each state variable's time series.
//! 3. Extract the dominant frequency bins and their amplitudes.
//! 4. Synthesise audio by summing sinusoids at those frequencies with
//!    amplitudes proportional to the FFT magnitudes.
//!
//! The result: audio whose spectrum IS the attractor's power spectrum.
//! Chaotic attractors produce rich, broadband spectra; limit cycles produce
//! sparse harmonic spectra.
//!
//! ## Usage
//!
//! ```rust
//! use math_sonify::spectral_composition::{SpectralComposer, SpectralConfig};
//!
//! let cfg = SpectralConfig::default();
//! let mut composer = SpectralComposer::new(cfg);
//! composer.ingest(&[1.0, 0.5, -0.3, -0.8, 0.2]);
//! let _ = composer.synthesise(44100, 512);
//! ```

/// Configuration for spectral composition.
#[derive(Debug, Clone)]
pub struct SpectralConfig {
    /// Number of trajectory samples to accumulate before FFT.
    pub window_size: usize,
    /// Number of dominant spectral bins to use for synthesis.
    pub num_partials: usize,
    /// Base frequency (Hz) for mapping FFT bin 1.
    pub base_frequency: f64,
    /// Maximum frequency (Hz) allowed for any partial.
    pub max_frequency: f64,
    /// Amplitude scale factor.
    pub amplitude_scale: f64,
    /// Smoothing: blend fraction of new spectrum into running average (0..1).
    pub spectral_smoothing: f64,
}

impl Default for SpectralConfig {
    fn default() -> Self {
        Self {
            window_size: 256,
            num_partials: 16,
            base_frequency: 55.0,
            max_frequency: 8000.0,
            amplitude_scale: 0.8,
            spectral_smoothing: 0.1,
        }
    }
}

/// A single spectral partial: a sinusoidal component.
#[derive(Debug, Clone)]
pub struct Partial {
    /// Frequency in Hz.
    pub frequency: f64,
    /// Amplitude (0..1).
    pub amplitude: f64,
    /// Current phase (radians, for stateful synthesis).
    pub phase: f64,
}

impl Partial {
    /// Advance this partial by one audio sample and return the sample value.
    pub fn tick(&mut self, sample_rate: f64) -> f64 {
        let sample = self.amplitude * self.phase.sin();
        self.phase += 2.0 * std::f64::consts::PI * self.frequency / sample_rate;
        // Wrap phase to avoid float drift
        if self.phase > std::f64::consts::TAU {
            self.phase -= std::f64::consts::TAU;
        }
        sample
    }
}

/// Power spectrum bin.
#[derive(Debug, Clone)]
pub struct SpectrumBin {
    /// Bin index.
    pub bin: usize,
    /// Frequency (Hz).
    pub frequency: f64,
    /// Magnitude (linear).
    pub magnitude: f64,
}

/// Spectral composition engine.
pub struct SpectralComposer {
    pub config: SpectralConfig,
    /// Circular buffer of incoming trajectory samples.
    buffer: Vec<f64>,
    /// Write index into buffer.
    write_idx: usize,
    /// Whether the buffer has been filled at least once.
    buffer_full: bool,
    /// Currently active partials (synthesised from most recent FFT).
    pub partials: Vec<Partial>,
    /// Smoothed power spectrum (bins).
    smoothed_spectrum: Vec<f64>,
}

impl SpectralComposer {
    pub fn new(config: SpectralConfig) -> Self {
        let sz = config.window_size;
        Self {
            buffer: vec![0.0; sz],
            write_idx: 0,
            buffer_full: false,
            partials: Vec::new(),
            smoothed_spectrum: vec![0.0; sz / 2],
            config,
        }
    }

    /// Ingest a batch of attractor samples (one state variable, e.g., x).
    pub fn ingest(&mut self, samples: &[f64]) {
        let sz = self.config.window_size;
        for &s in samples {
            self.buffer[self.write_idx] = s;
            self.write_idx = (self.write_idx + 1) % sz;
            if self.write_idx == 0 {
                self.buffer_full = true;
            }
        }
        if self.buffer_full || self.write_idx >= sz / 2 {
            self.update_spectrum();
        }
    }

    /// Update the smoothed power spectrum from the current buffer.
    fn update_spectrum(&mut self) {
        let sz = self.config.window_size;
        // Apply Hann window then compute DFT magnitude (no external crate needed)
        let windowed: Vec<f64> = (0..sz)
            .map(|i| {
                let w = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (sz - 1) as f64).cos());
                // Read from circular buffer with correct offset
                let buf_idx = (self.write_idx + i) % sz;
                self.buffer[buf_idx] * w
            })
            .collect();

        let n_bins = sz / 2;
        let alpha = self.config.spectral_smoothing;

        for k in 0..n_bins {
            // DFT magnitude at bin k (O(N²) but window_size is small)
            let mut re = 0.0_f64;
            let mut im = 0.0_f64;
            for n in 0..sz {
                let angle = -2.0 * std::f64::consts::PI * k as f64 * n as f64 / sz as f64;
                re += windowed[n] * angle.cos();
                im += windowed[n] * angle.sin();
            }
            let mag = (re * re + im * im).sqrt() / sz as f64;
            // Exponential smoothing
            self.smoothed_spectrum[k] = (1.0 - alpha) * self.smoothed_spectrum[k] + alpha * mag;
        }
    }

    /// Extract the top-N spectral bins from the smoothed spectrum.
    pub fn dominant_bins(&self, sample_rate: f64) -> Vec<SpectrumBin> {
        let sz = self.config.window_size;
        let n_bins = sz / 2;
        let mut bins: Vec<SpectrumBin> = (1..n_bins) // skip DC
            .map(|k| {
                let freq = k as f64 * sample_rate / sz as f64;
                SpectrumBin {
                    bin: k,
                    frequency: freq,
                    magnitude: self.smoothed_spectrum[k],
                }
            })
            .filter(|b| b.frequency <= self.config.max_frequency)
            .collect();

        bins.sort_by(|a, b| b.magnitude.partial_cmp(&a.magnitude).unwrap());
        bins.truncate(self.config.num_partials);
        bins
    }

    /// Refresh the partial set from the current spectrum.
    pub fn refresh_partials(&mut self, sample_rate: f64) {
        let bins = self.dominant_bins(sample_rate);
        let max_mag = bins
            .iter()
            .map(|b| b.magnitude)
            .fold(0.0_f64, f64::max)
            .max(1e-12);

        // Keep existing phases to avoid clicks; match by bin index
        let old_partials: std::collections::HashMap<usize, f64> = self
            .partials
            .iter()
            .map(|p| {
                // Map frequency back to approximate bin index
                let bin = (p.frequency * self.config.window_size as f64 / sample_rate) as usize;
                (bin, p.phase)
            })
            .collect();

        self.partials = bins
            .iter()
            .map(|b| {
                let amplitude =
                    (b.magnitude / max_mag * self.config.amplitude_scale).clamp(0.0, 1.0);
                let phase = old_partials.get(&b.bin).copied().unwrap_or(0.0);
                Partial {
                    frequency: b.frequency,
                    amplitude,
                    phase,
                }
            })
            .collect();
    }

    /// Synthesise `num_samples` audio samples by summing active partials.
    pub fn synthesise(&mut self, sample_rate: u32, num_samples: usize) -> Vec<f32> {
        let sr = sample_rate as f64;
        self.refresh_partials(sr);

        (0..num_samples)
            .map(|_| {
                let sample: f64 = self.partials.iter_mut().map(|p| p.tick(sr)).sum();
                // Soft clip
                let clipped = sample.tanh();
                clipped as f32
            })
            .collect()
    }

    /// Render an ASCII bar chart of the current power spectrum.
    pub fn spectrum_bars(&self, width: usize) -> String {
        let n_bins = self.smoothed_spectrum.len().min(width);
        let max = self.smoothed_spectrum[1..n_bins]
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max)
            .max(1e-12);
        let bar_chars = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let bars: String = (1..n_bins)
            .map(|k| {
                let norm = self.smoothed_spectrum[k] / max;
                let idx = (norm * (bar_chars.len() - 1) as f64) as usize;
                bar_chars[idx.min(bar_chars.len() - 1)]
            })
            .collect();
        format!("[{}]", bars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_wave(freq: f64, sr: f64, n: usize) -> Vec<f64> {
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / sr).sin())
            .collect()
    }

    #[test]
    fn ingest_and_synthesise() {
        let cfg = SpectralConfig {
            window_size: 64,
            num_partials: 4,
            ..Default::default()
        };
        let mut composer = SpectralComposer::new(cfg);
        let signal = sine_wave(440.0, 44100.0, 128);
        composer.ingest(&signal);
        let audio = composer.synthesise(44100, 256);
        assert_eq!(audio.len(), 256);
        assert!(audio.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn dominant_bins_at_440() {
        let cfg = SpectralConfig {
            window_size: 256,
            num_partials: 8,
            ..Default::default()
        };
        let mut composer = SpectralComposer::new(cfg);
        let signal = sine_wave(440.0, 44100.0, 512);
        composer.ingest(&signal);
        let bins = composer.dominant_bins(44100.0);
        assert!(!bins.is_empty());
        // Top bin should be near 440 Hz
        let top_freq = bins[0].frequency;
        // With window_size=256 and sr=44100, bin resolution ≈ 172 Hz; bin near 440 is bin 2 or 3
        assert!(top_freq > 100.0 && top_freq < 1000.0,
            "top bin freq should be in range: {top_freq}");
    }

    #[test]
    fn synthesised_audio_soft_clipped() {
        let mut composer = SpectralComposer::new(SpectralConfig::default());
        // Ingest many ones to produce a large magnitude
        composer.ingest(&vec![1.0; 512]);
        let audio = composer.synthesise(44100, 128);
        for s in &audio {
            assert!(s.abs() <= 1.0 + 1e-6, "sample out of tanh range: {s}");
        }
    }

    #[test]
    fn spectrum_bars_width() {
        let composer = SpectralComposer::new(SpectralConfig { window_size: 64, ..Default::default() });
        let bars = composer.spectrum_bars(30);
        // Should return a string containing at least some characters
        assert!(bars.len() > 2);
    }
}
