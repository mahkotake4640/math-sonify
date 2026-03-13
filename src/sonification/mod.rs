pub mod direct;
pub mod orbital;
pub mod granular;
pub mod spectral;

pub use direct::DirectMapping;
pub use orbital::OrbitalResonance;
pub use granular::GranularMapping;
pub use spectral::SpectralMapping;

use crate::config::SonificationConfig;

/// Parameters computed from the dynamical system state, consumed by the audio thread.
#[derive(Clone, Default)]
pub struct AudioParams {
    /// Oscillator frequencies for voices (Hz).
    pub freqs: [f32; 4],
    /// Amplitudes for each voice (0..1).
    pub amps: [f32; 4],
    /// Filter cutoff frequency (Hz).
    pub filter_cutoff: f32,
    /// Filter Q.
    pub filter_q: f32,
    /// Stereo pan for voices (-1..1).
    pub pans: [f32; 4],
    /// Granular: spawn rate and base frequency.
    pub grain_spawn_rate: f32,
    pub grain_base_freq: f32,
    pub grain_freq_spread: f32,
    /// Spectral: amplitude of each harmonic partial (up to 32).
    pub partials: [f32; 32],
    pub partials_base_freq: f32,
    /// Active sonification mode — tells the audio thread which path to render.
    pub mode: SonifMode,
    /// Master gain scalar (already accounts for per-mode normalization).
    pub gain: f32,
    /// Semitone transpose applied to all voices.
    pub transpose_semitones: f32,
    /// Semitone offsets for 3 chord voices from voice[0] (0 = off).
    pub chord_intervals: [f32; 3],
    /// Per-voice amplitude mix 0..1.
    pub voice_levels: [f32; 4],
    /// Frequency glide time in milliseconds.
    pub portamento_ms: f32,
    /// Chaos estimate 0..1 for display.
    pub chaos_level: f32,
    /// Master volume (0..1).
    pub master_volume: f32,
    /// Reverb wet mix (0..1).
    pub reverb_wet: f32,
    /// Delay time in milliseconds.
    pub delay_ms: f32,
    /// Delay feedback (0..1).
    pub delay_feedback: f32,
}

pub fn chord_intervals_for(mode: &str) -> [f32; 3] {
    match mode {
        "major"  => [4.0, 7.0, 0.0],
        "minor"  => [3.0, 7.0, 0.0],
        "power"  => [7.0, 12.0, 0.0],
        "sus2"   => [2.0, 7.0, 0.0],
        "octave" => [12.0, 24.0, 0.0],
        "dom7"   => [4.0, 7.0, 10.0],
        _        => [0.0, 0.0, 0.0],
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum SonifMode { #[default] Direct, Orbital, Granular, Spectral }

impl std::fmt::Display for SonifMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => write!(f, "Direct"),
            Self::Orbital => write!(f, "Orbital"),
            Self::Granular => write!(f, "Granular"),
            Self::Spectral => write!(f, "Spectral"),
        }
    }
}

/// Musical scale quantization.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Scale { #[default] Pentatonic, Chromatic, JustIntonation, Microtonal }

/// Semitone intervals for each scale, relative to root.
fn scale_intervals(scale: Scale) -> &'static [f32] {
    match scale {
        Scale::Pentatonic =>      &[0.0, 2.0, 4.0, 7.0, 9.0],
        Scale::Chromatic =>       &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0],
        Scale::JustIntonation =>  &[0.0, 2.039, 3.863, 4.980, 7.020, 8.841, 10.884], // just major
        Scale::Microtonal =>      &[0.0, 0.75, 1.5, 2.25, 3.0, 3.75, 4.5, 5.25, 6.0, 6.75, 7.5, 8.25, 9.0],
    }
}

/// Quantize a continuous value [0..1] to a frequency on the given scale.
pub fn quantize_to_scale(t: f32, base_hz: f32, octave_range: f32, scale: Scale) -> f32 {
    let intervals = scale_intervals(scale);
    let n = intervals.len() as f32;
    // Map t to a position in the scale across octave_range octaves
    let total_steps = octave_range * n;
    let step_float = (t.clamp(0.0, 1.0) * total_steps) as usize;
    let octave = step_float / intervals.len();
    let degree = step_float % intervals.len();
    let semitones = octave as f32 * 12.0 + intervals[degree];
    base_hz * 2.0f32.powf(semitones / 12.0)
}

pub trait Sonification: Send {
    /// Map the system state to audio parameters.
    fn map(&mut self, state: &[f64], speed: f64, config: &SonificationConfig) -> AudioParams;
}
