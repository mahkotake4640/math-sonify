//! Physical modeling synthesis modes.
//!
//! This module wraps the lower-level [`KarplusStrong`] and [`WaveguideString`]
//! DSP primitives (in `src/synth/`) in higher-level "instrument" abstractions
//! that accept normalised state vectors from the dynamical system and produce
//! audio frames.
//!
//! # Synthesis modes
//!
//! | Mode | DSP core | Character |
//! |------|----------|-----------|
//! | [`PluckedString`] | Karplus-Strong | Bright plucked string, guitar/harp |
//! | [`TubeResonator`] | Waveguide (two-delay) + resonator | Wind instrument, tube resonance |
//!
//! Both instruments expose a common [`PhysicalSynth`] trait so the audio
//! engine can dispatch dynamically.

#![allow(dead_code)]

use crate::synth::{KarplusStrong, ResonatorBank, WaveguideString};

// ── Trait ─────────────────────────────────────────────────────────────────────

/// Common interface for physical modeling synthesis modes.
pub trait PhysicalSynth: Send {
    /// Map a normalised state vector (values in roughly `[-1, 1]`) to synthesis
    /// parameters and produce the next audio sample.
    ///
    /// - `state[0]` → pitch / frequency
    /// - `state[1]` → brightness / timbre
    /// - `state[2]` → excitation / pluck strength (if present)
    fn next_sample(&mut self, state: &[f64], sample_rate: f32) -> f32;

    /// True if this synth is currently active (producing non-zero output).
    fn is_active(&self) -> bool;

    /// Force re-excitation (e.g. when the attractor crosses a threshold).
    fn excite(&mut self, freq_hz: f32, sample_rate: f32);

    /// Set master output volume (0–1).
    fn set_volume(&mut self, vol: f32);
}

// ── Plucked string ────────────────────────────────────────────────────────────

/// Karplus-Strong plucked string instrument driven by ODE state.
///
/// The dynamical system state modulates:
/// - `state[0]` → pitch (mapped to a frequency range)
/// - `state[1]` → brightness (IIR coefficient)
/// - Automatic re-excitation when the pitch changes by more than a threshold
pub struct PluckedString {
    ks: KarplusStrong,
    /// Minimum frequency in Hz.
    freq_min: f32,
    /// Maximum frequency in Hz.
    freq_max: f32,
    last_freq: f32,
    /// Frequency change that triggers a re-excitation (cents).
    retrigger_cents: f32,
    /// Excitation cooldown in samples to avoid clicking.
    cooldown: u32,
    cooldown_remaining: u32,
    volume: f32,
}

impl PluckedString {
    /// Create a plucked string with the given frequency range.
    pub fn new(freq_min: f32, freq_max: f32, sample_rate: f32) -> Self {
        Self {
            ks: KarplusStrong::new(freq_min.max(10.0), sample_rate),
            freq_min: freq_min.max(10.0),
            freq_max: freq_max.max(freq_min + 1.0),
            last_freq: 0.0,
            retrigger_cents: 200.0, // 2 semitones
            cooldown: (sample_rate * 0.05) as u32, // 50 ms
            cooldown_remaining: 0,
            volume: 0.7,
        }
    }

    /// Map a normalised value in `[-1, 1]` to a frequency in Hz (log scale).
    fn map_freq(&self, x: f64) -> f32 {
        let t = ((x + 1.0) * 0.5).clamp(0.0, 1.0) as f32;
        let log_min = self.freq_min.ln();
        let log_max = self.freq_max.ln();
        (log_min + t * (log_max - log_min)).exp()
    }

    /// Cents distance between two frequencies.
    fn cents_diff(f1: f32, f2: f32) -> f32 {
        if f1 <= 0.0 || f2 <= 0.0 {
            return f32::MAX;
        }
        (1200.0 * (f2 / f1).log2()).abs()
    }
}

impl PhysicalSynth for PluckedString {
    fn next_sample(&mut self, state: &[f64], sample_rate: f32) -> f32 {
        if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
        }

        let freq = self
            .map_freq(state.first().copied().unwrap_or(0.0));

        // Re-excite if pitch has moved significantly.
        if self.cooldown_remaining == 0
            && Self::cents_diff(self.last_freq, freq) > self.retrigger_cents
        {
            self.ks.trigger(freq, sample_rate);
            self.last_freq = freq;
            self.cooldown_remaining = self.cooldown;
        }

        // Modulate brightness from state[1].
        if let Some(&s1) = state.get(1) {
            let b = ((s1 + 1.0) * 0.5).clamp(0.0, 1.0) as f32 * 0.8;
            self.ks.brightness = b;
        }

        self.ks.volume = self.volume;
        self.ks.next_sample()
    }

    fn is_active(&self) -> bool {
        self.ks.active
    }

    fn excite(&mut self, freq_hz: f32, sample_rate: f32) {
        self.ks.trigger(freq_hz, sample_rate);
        self.last_freq = freq_hz;
        self.cooldown_remaining = self.cooldown;
    }

    fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
    }
}

// ── Tube resonator ────────────────────────────────────────────────────────────

/// Cylindrical/conical tube resonator using a bidirectional waveguide model.
///
/// Models a wind-instrument resonating column.  The ODE state modulates:
/// - `state[0]` → resonant frequency (embouchure / fingering)
/// - `state[1]` → damping (open vs. stopped)
/// - `state[2]` → excitation pressure (breathiness)
///
/// A [`ResonatorBank`] adds body resonance on top of the waveguide output.
pub struct TubeResonator {
    wg: WaveguideString,
    resonators: ResonatorBank,
    /// Frequency range.
    freq_min: f32,
    freq_max: f32,
    /// Continuous excitation level derived from `state[2]`.
    excite_level: f32,
    volume: f32,
    sample_rate: f32,
    /// Noise state for breath excitation.
    noise_seed: u64,
}

impl TubeResonator {
    /// Create a tube resonator for the given frequency range.
    pub fn new(freq_min: f32, freq_max: f32, sample_rate: f32) -> Self {
        let mut wg = WaveguideString::new(sample_rate);
        wg.set_freq(freq_min);
        wg.damping = 0.998;
        wg.brightness = 0.2; // bright tube
        wg.dispersion = 0.0; // ideal tube (no stiffness)

        // Initialise resonator bank at base frequency.
        let mut resonators = ResonatorBank::new(sample_rate);
        resonators.tune_to_scale(freq_min, 2.0, &[0.0, 7.0, 12.0]);
        resonators.q = 20.0;

        Self {
            wg,
            resonators,
            freq_min: freq_min.max(20.0),
            freq_max: freq_max.max(freq_min + 1.0),
            excite_level: 0.0,
            volume: 0.6,
            sample_rate,
            noise_seed: 0xDEAD_CAFE_1234_5678,
        }
    }

    fn map_freq(&self, x: f64) -> f32 {
        let t = ((x + 1.0) * 0.5).clamp(0.0, 1.0) as f32;
        let log_min = self.freq_min.ln();
        let log_max = self.freq_max.ln();
        (log_min + t * (log_max - log_min)).exp()
    }

    /// Produce a breath noise sample scaled by `level`.
    fn breath_noise(&mut self, level: f32) -> f32 {
        self.noise_seed = self
            .noise_seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let n = (self.noise_seed >> 33) as f32 / (1u64 << 31) as f32 * 2.0 - 1.0;
        n * level * 0.1
    }
}

impl PhysicalSynth for TubeResonator {
    fn next_sample(&mut self, state: &[f64], _sample_rate: f32) -> f32 {
        // Map frequency.
        let freq = self.map_freq(state.first().copied().unwrap_or(0.0));
        self.wg.set_freq(freq);

        // Map damping from state[1].
        if let Some(&s1) = state.get(1) {
            let d = 0.990 + ((s1 + 1.0) * 0.5).clamp(0.0, 1.0) as f32 * 0.008;
            self.wg.damping = d;
        }

        // Breath pressure from state[2] — continuous excitation.
        let pressure = state.get(2).copied().unwrap_or(0.5);
        self.excite_level = ((pressure + 1.0) * 0.5).clamp(0.0, 1.0) as f32;

        // Inject breath noise into the waveguide.
        if self.excite_level > 0.01 {
            let noise = self.breath_noise(self.excite_level);
            // Pulse the excite flag when noise is large enough.
            if noise.abs() > 0.05 {
                self.wg.excite = true;
                self.wg.excite_pos = 0.1; // near the mouthpiece
            }
        }

        let wg_out = self.wg.next_sample();
        let (res_l, res_r) = self.resonators.process(wg_out);
        let resonated = (res_l + res_r) * 0.5;

        (wg_out * 0.7 + resonated * 0.3) * self.volume
    }

    fn is_active(&self) -> bool {
        // The tube resonator is continuously active when driven by breath.
        self.excite_level > 0.001
    }

    fn excite(&mut self, freq_hz: f32, _sample_rate: f32) {
        self.wg.set_freq(freq_hz);
        self.wg.excite = true;
        self.excite_level = 0.5;
    }

    fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
    }
}

// ── FM Synthesis ──────────────────────────────────────────────────────────────

/// Configuration for DX7-style FM synthesis.
///
/// Frequency modulation synthesis using a single carrier and modulator operator.
/// The modulator adds sidebands to the carrier, controlled by `modulation_index`.
#[derive(Debug, Clone)]
pub struct FmConfig {
    /// Carrier-to-fundamental frequency ratio.
    pub carrier_ratio: f32,
    /// Modulator-to-fundamental frequency ratio.
    pub modulator_ratio: f32,
    /// Modulation index: controls FM sideband depth (0 = pure sine, 4 = rich spectrum).
    pub modulation_index: f32,
}

impl Default for FmConfig {
    fn default() -> Self {
        Self {
            carrier_ratio: 1.0,
            modulator_ratio: 2.0,
            modulation_index: 2.0,
        }
    }
}

/// A simple ADSR envelope generator.
///
/// Cycles through Attack, Decay, Sustain (held until released), and Release.
/// The envelope is driven by sample count rather than wall-clock time.
#[derive(Debug, Clone)]
pub struct AdsrEnvelope {
    /// Attack length in samples.
    pub attack_samples: u32,
    /// Decay length in samples.
    pub decay_samples: u32,
    /// Sustain level (0..=1).
    pub sustain_level: f32,
    /// Release length in samples.
    pub release_samples: u32,

    // Private state
    phase: AdsrPhase,
    sample_counter: u32,
    current_level: f32,
    release_start_level: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AdsrPhase {
    Attack,
    Decay,
    Sustain,
    Release,
    Idle,
}

impl AdsrEnvelope {
    /// Create a new ADSR envelope (starts idle until triggered).
    pub fn new(attack_samples: u32, decay_samples: u32, sustain_level: f32, release_samples: u32) -> Self {
        Self {
            attack_samples,
            decay_samples,
            sustain_level: sustain_level.clamp(0.0, 1.0),
            release_samples,
            phase: AdsrPhase::Idle,
            sample_counter: 0,
            current_level: 0.0,
            release_start_level: 0.0,
        }
    }

    /// Trigger the envelope (starts Attack phase).
    pub fn trigger(&mut self) {
        self.phase = AdsrPhase::Attack;
        self.sample_counter = 0;
    }

    /// Begin release phase.
    pub fn release(&mut self) {
        self.release_start_level = self.current_level;
        self.phase = AdsrPhase::Release;
        self.sample_counter = 0;
    }

    /// Return whether the envelope has fully decayed to idle.
    pub fn is_idle(&self) -> bool {
        self.phase == AdsrPhase::Idle
    }

    /// Advance one sample and return the current amplitude (0..=1).
    pub fn next_sample(&mut self) -> f32 {
        match self.phase {
            AdsrPhase::Idle => {
                self.current_level = 0.0;
            }
            AdsrPhase::Attack => {
                if self.attack_samples == 0 {
                    self.current_level = 1.0;
                    self.phase = AdsrPhase::Decay;
                    self.sample_counter = 0;
                } else {
                    self.current_level = self.sample_counter as f32 / self.attack_samples as f32;
                    self.sample_counter += 1;
                    if self.sample_counter >= self.attack_samples {
                        self.current_level = 1.0;
                        self.phase = AdsrPhase::Decay;
                        self.sample_counter = 0;
                    }
                }
            }
            AdsrPhase::Decay => {
                if self.decay_samples == 0 {
                    self.current_level = self.sustain_level;
                    self.phase = AdsrPhase::Sustain;
                } else {
                    let t = self.sample_counter as f32 / self.decay_samples as f32;
                    self.current_level = 1.0 - t * (1.0 - self.sustain_level);
                    self.sample_counter += 1;
                    if self.sample_counter >= self.decay_samples {
                        self.current_level = self.sustain_level;
                        self.phase = AdsrPhase::Sustain;
                        self.sample_counter = 0;
                    }
                }
            }
            AdsrPhase::Sustain => {
                self.current_level = self.sustain_level;
            }
            AdsrPhase::Release => {
                if self.release_samples == 0 {
                    self.current_level = 0.0;
                    self.phase = AdsrPhase::Idle;
                } else {
                    let t = self.sample_counter as f32 / self.release_samples as f32;
                    self.current_level = self.release_start_level * (1.0 - t);
                    self.sample_counter += 1;
                    if self.sample_counter >= self.release_samples {
                        self.current_level = 0.0;
                        self.phase = AdsrPhase::Idle;
                    }
                }
            }
        }
        self.current_level.clamp(0.0, 1.0)
    }
}

impl Default for AdsrEnvelope {
    fn default() -> Self {
        Self::new(441, 4410, 0.7, 8820) // 10ms attack, 100ms decay, 0.7 sustain, 200ms release at 44100 Hz
    }
}

/// DX7-style FM synthesizer implementing [`PhysicalSynth`].
///
/// Maps ODE state to carrier frequency (log-mapped), modulation index, and
/// amplitude envelope (ADSR). Uses a single carrier + modulator operator pair.
///
/// State mapping:
/// - `state[0]` → carrier frequency (log-mapped over `freq_min..freq_max`)
/// - `state[1]` → modulation index (0→4)
/// - `state[2]` → ADSR decay driven by normalized value
pub struct FmSynth {
    config: FmConfig,
    sample_rate: f32,
    freq_min: f32,
    freq_max: f32,
    // Synthesis state
    carrier_phase: f32,
    modulator_phase: f32,
    carrier_freq: f32,
    modulation_index: f32,
    envelope: AdsrEnvelope,
    volume: f32,
    /// Decrement decay counter when env drops below threshold
    last_excite_value: f32,
}

impl FmSynth {
    /// Create a new FM synthesizer.
    pub fn new(config: FmConfig, sample_rate: f32) -> Self {
        let mut env = AdsrEnvelope::new(
            (sample_rate * 0.01) as u32, // 10ms attack
            (sample_rate * 0.1) as u32,  // 100ms decay
            0.7,
            (sample_rate * 0.2) as u32,  // 200ms release
        );
        env.trigger();
        Self {
            config,
            sample_rate: sample_rate.max(1.0),
            freq_min: 80.0,
            freq_max: 1200.0,
            carrier_phase: 0.0,
            modulator_phase: 0.0,
            carrier_freq: 440.0,
            modulation_index: 2.0,
            envelope: env,
            volume: 0.7,
            last_excite_value: 0.0,
        }
    }

    fn map_freq(&self, x: f64) -> f32 {
        let t = ((x + 1.0) * 0.5).clamp(0.0, 1.0) as f32;
        let log_min = self.freq_min.ln();
        let log_max = self.freq_max.ln();
        (log_min + t * (log_max - log_min)).exp()
    }
}

impl PhysicalSynth for FmSynth {
    fn next_sample(&mut self, state: &[f64], _sample_rate: f32) -> f32 {
        let sr = self.sample_rate;

        // Map state[0] → carrier frequency (log-mapped)
        if let Some(&s0) = state.first() {
            self.carrier_freq = self.map_freq(s0);
        }

        // Map state[1] → modulation index (0→4)
        if let Some(&s1) = state.get(1) {
            self.modulation_index = (((s1 + 1.0) * 0.5).clamp(0.0, 1.0) * 4.0) as f32;
        }

        // Map state[2] → ADSR decay trigger: large changes re-trigger envelope
        if let Some(&s2) = state.get(2) {
            let val = s2 as f32;
            if (val - self.last_excite_value).abs() > 2.0 {
                self.envelope.trigger();
                self.last_excite_value = val;
            }
        }

        // FM synthesis: modulator drives carrier phase
        let mod_freq = self.carrier_freq * self.config.modulator_ratio;
        let carrier_freq = self.carrier_freq * self.config.carrier_ratio;

        let mod_sample = (self.modulator_phase * std::f32::consts::TAU).sin()
            * self.modulation_index;

        let carrier_sample = ((self.carrier_phase * std::f32::consts::TAU) + mod_sample).sin();

        // Advance phases
        self.modulator_phase = (self.modulator_phase + mod_freq / sr).fract();
        self.carrier_phase = (self.carrier_phase + carrier_freq / sr).fract();

        let env_level = self.envelope.next_sample();
        carrier_sample * env_level * self.volume
    }

    fn is_active(&self) -> bool {
        !self.envelope.is_idle()
    }

    fn excite(&mut self, freq_hz: f32, _sample_rate: f32) {
        self.carrier_freq = freq_hz;
        self.envelope.trigger();
    }

    fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Available physical synthesis modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalMode {
    PluckedString,
    TubeResonator,
    /// DX7-style FM synthesis.
    Fm,
}

/// Construct the appropriate physical synth.
pub fn build_physical_synth(
    mode: PhysicalMode,
    freq_min: f32,
    freq_max: f32,
    sample_rate: f32,
) -> Box<dyn PhysicalSynth> {
    match mode {
        PhysicalMode::PluckedString => {
            Box::new(PluckedString::new(freq_min, freq_max, sample_rate))
        }
        PhysicalMode::TubeResonator => {
            Box::new(TubeResonator::new(freq_min, freq_max, sample_rate))
        }
        PhysicalMode::Fm => {
            let mut s = FmSynth::new(FmConfig::default(), sample_rate);
            s.freq_min = freq_min.max(20.0);
            s.freq_max = freq_max.max(freq_min + 1.0);
            Box::new(s)
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn test_plucked_string_produces_output_after_excite() {
        let mut ps = PluckedString::new(80.0, 1200.0, SR);
        ps.excite(440.0, SR);
        let mut max = 0.0_f32;
        let state = [0.0f64, 0.0, 0.0];
        for _ in 0..4410 {
            let s = ps.next_sample(&state, SR);
            max = max.max(s.abs());
        }
        assert!(max > 0.0, "PluckedString should produce output after excite");
    }

    #[test]
    fn test_plucked_string_output_finite() {
        let mut ps = PluckedString::new(80.0, 1200.0, SR);
        ps.excite(220.0, SR);
        let state = [0.0f64, 0.5, -0.5];
        for i in 0..22050 {
            let s = ps.next_sample(&state, SR);
            assert!(s.is_finite(), "non-finite at sample {i}");
        }
    }

    #[test]
    fn test_tube_resonator_produces_output() {
        let mut tr = TubeResonator::new(60.0, 800.0, SR);
        tr.excite(220.0, SR);
        let state = [0.0f64, 0.0, 0.5];
        let mut max = 0.0_f32;
        for _ in 0..4410 {
            max = max.max(tr.next_sample(&state, SR).abs());
        }
        assert!(max > 0.0, "TubeResonator should produce output after excite");
    }

    #[test]
    fn test_tube_resonator_output_finite() {
        let mut tr = TubeResonator::new(60.0, 800.0, SR);
        tr.excite(110.0, SR);
        let state = [0.3f64, -0.2, 0.8];
        for i in 0..22050 {
            let s = tr.next_sample(&state, SR);
            assert!(s.is_finite(), "non-finite at sample {i}");
        }
    }

    #[test]
    fn test_build_physical_synth_factory() {
        let mut ps = build_physical_synth(PhysicalMode::PluckedString, 80.0, 1200.0, SR);
        ps.excite(440.0, SR);
        let state = [0.0f64];
        let _ = ps.next_sample(&state, SR);

        let mut tr = build_physical_synth(PhysicalMode::TubeResonator, 60.0, 800.0, SR);
        tr.excite(220.0, SR);
        let _ = tr.next_sample(&state, SR);
    }
}
