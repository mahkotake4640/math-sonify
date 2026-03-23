//! ADSR and multi-stage envelope generators with LFO modulation.

use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Current stage of an ADSR/AHDSR envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeStage {
    Attack,
    Decay,
    Sustain,
    Release,
    Hold,
    Delay,
    Done,
}

/// Shape applied to the linear 0→1 ramp within each envelope segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurveType {
    Linear,
    Exponential,
    Logarithmic,
    SCurve,
}

// ---------------------------------------------------------------------------
// Curve helper
// ---------------------------------------------------------------------------

/// Map a linear `t ∈ [0, 1]` through the given curve shape, returning a value
/// also in `[0, 1]`.
pub fn apply_curve(t: f64, curve: CurveType) -> f64 {
    let t = t.clamp(0.0, 1.0);
    match curve {
        CurveType::Linear => t,
        CurveType::Exponential => t * t,
        CurveType::Logarithmic => {
            if t <= 0.0 {
                0.0
            } else {
                (1.0 + t * (std::f64::consts::E - 1.0)).ln()
            }
        }
        CurveType::SCurve => {
            // Smoothstep: 3t² − 2t³
            t * t * (3.0 - 2.0 * t)
        }
    }
}

// ---------------------------------------------------------------------------
// AdsrEnvelope
// ---------------------------------------------------------------------------

/// Standard four-stage ADSR envelope.
#[derive(Debug, Clone)]
pub struct AdsrEnvelope {
    pub attack_ms: f64,
    pub decay_ms: f64,
    /// Sustain level in [0, 1].
    pub sustain_level: f64,
    pub release_ms: f64,
    pub curve: CurveType,
}

impl AdsrEnvelope {
    /// Create a new ADSR envelope with linear curves.
    pub fn new(attack_ms: f64, decay_ms: f64, sustain_level: f64, release_ms: f64) -> Self {
        Self {
            attack_ms,
            decay_ms,
            sustain_level: sustain_level.clamp(0.0, 1.0),
            release_ms,
            curve: CurveType::Linear,
        }
    }

    /// Determine the current envelope stage at `time_ms`.
    pub fn current_stage(
        &self,
        time_ms: f64,
        gate_on: bool,
        gate_off_at: Option<f64>,
    ) -> EnvelopeStage {
        let off_t = gate_off_at.unwrap_or(f64::INFINITY);

        if !gate_on && gate_off_at.is_none() {
            return EnvelopeStage::Done;
        }

        if time_ms < off_t {
            // Gate is still on.
            if time_ms < self.attack_ms {
                EnvelopeStage::Attack
            } else if time_ms < self.attack_ms + self.decay_ms {
                EnvelopeStage::Decay
            } else {
                EnvelopeStage::Sustain
            }
        } else {
            // Gate released.
            let release_end = off_t + self.release_ms;
            if time_ms < release_end {
                EnvelopeStage::Release
            } else {
                EnvelopeStage::Done
            }
        }
    }

    /// Compute the amplitude [0, 1] at `time_ms`.
    pub fn sample_at(&self, time_ms: f64, gate_on: bool, gate_off_at: Option<f64>) -> f64 {
        let stage = self.current_stage(time_ms, gate_on, gate_off_at);
        let off_t = gate_off_at.unwrap_or(f64::INFINITY);

        match stage {
            EnvelopeStage::Attack => {
                let t = time_ms / self.attack_ms.max(1e-9);
                apply_curve(t, self.curve)
            }
            EnvelopeStage::Decay => {
                let t = (time_ms - self.attack_ms) / self.decay_ms.max(1e-9);
                1.0 - apply_curve(t, self.curve) * (1.0 - self.sustain_level)
            }
            EnvelopeStage::Sustain => self.sustain_level,
            EnvelopeStage::Release => {
                let t = (time_ms - off_t) / self.release_ms.max(1e-9);
                self.sustain_level * (1.0 - apply_curve(t, self.curve))
            }
            EnvelopeStage::Done => 0.0,
            // Unused by ADSR.
            EnvelopeStage::Hold | EnvelopeStage::Delay => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// AhdsrEnvelope
// ---------------------------------------------------------------------------

/// Five-stage Attack-Hold-Decay-Sustain-Release envelope.
#[derive(Debug, Clone)]
pub struct AhdsrEnvelope {
    pub attack_ms: f64,
    pub hold_ms: f64,
    pub decay_ms: f64,
    pub sustain_level: f64,
    pub release_ms: f64,
    pub curve: CurveType,
}

impl AhdsrEnvelope {
    /// Create a new AHDSR envelope.
    pub fn new(
        attack_ms: f64,
        hold_ms: f64,
        decay_ms: f64,
        sustain_level: f64,
        release_ms: f64,
    ) -> Self {
        Self {
            attack_ms,
            hold_ms,
            decay_ms,
            sustain_level: sustain_level.clamp(0.0, 1.0),
            release_ms,
            curve: CurveType::Linear,
        }
    }

    /// Compute the amplitude at `time_ms`. Gate-off time is given by `gate_off_at`.
    pub fn sample_at(&self, time_ms: f64, gate_off_at: Option<f64>) -> f64 {
        let off_t = gate_off_at.unwrap_or(f64::INFINITY);

        if time_ms < off_t {
            let t_attack_end = self.attack_ms;
            let t_hold_end = t_attack_end + self.hold_ms;
            let t_decay_end = t_hold_end + self.decay_ms;

            if time_ms < t_attack_end {
                let t = time_ms / self.attack_ms.max(1e-9);
                apply_curve(t, self.curve)
            } else if time_ms < t_hold_end {
                1.0 // hold at peak
            } else if time_ms < t_decay_end {
                let t = (time_ms - t_hold_end) / self.decay_ms.max(1e-9);
                1.0 - apply_curve(t, self.curve) * (1.0 - self.sustain_level)
            } else {
                self.sustain_level
            }
        } else {
            let t = (time_ms - off_t) / self.release_ms.max(1e-9);
            if t >= 1.0 {
                0.0
            } else {
                self.sustain_level * (1.0 - apply_curve(t, self.curve))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MultiStageEnvelope
// ---------------------------------------------------------------------------

/// An arbitrary N-stage envelope with optional looping.
#[derive(Debug, Clone, Default)]
pub struct MultiStageEnvelope {
    /// (duration_ms, target_level) pairs.
    pub stages: Vec<(f64, f64)>,
    /// If set, loop back to this stage index on completion.
    pub loop_start: Option<usize>,
}

impl MultiStageEnvelope {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a stage that ramps to `target_level` over `duration_ms`.
    pub fn add_stage(&mut self, duration_ms: f64, target_level: f64) {
        self.stages.push((duration_ms, target_level.clamp(0.0, 1.0)));
    }

    /// Set the stage to loop back to after the last stage.
    pub fn set_loop(&mut self, from_stage: usize) {
        self.loop_start = Some(from_stage);
    }

    /// Sample the envelope at `time_ms` with linear interpolation and optional looping.
    pub fn sample_at(&self, time_ms: f64) -> f64 {
        if self.stages.is_empty() {
            return 0.0;
        }

        let total_duration: f64 = self.stages.iter().map(|(d, _)| d).sum();

        // Compute effective time, applying looping if configured.
        let effective_time = if let Some(loop_start) = self.loop_start {
            let pre_loop_duration: f64 = self.stages[..loop_start.min(self.stages.len())]
                .iter()
                .map(|(d, _)| d)
                .sum();
            let loop_duration = total_duration - pre_loop_duration;

            if time_ms < pre_loop_duration || loop_duration <= 0.0 {
                time_ms
            } else {
                let t_in_loop = (time_ms - pre_loop_duration) % loop_duration;
                pre_loop_duration + t_in_loop
            }
        } else {
            time_ms.min(total_duration)
        };

        // Walk through stages.
        let mut elapsed = 0.0_f64;
        let mut prev_level = 0.0_f64;

        for (duration, target) in &self.stages {
            let stage_end = elapsed + duration;
            if effective_time <= stage_end {
                let t = if *duration <= 0.0 {
                    1.0
                } else {
                    (effective_time - elapsed) / duration
                };
                return prev_level + (target - prev_level) * t.clamp(0.0, 1.0);
            }
            elapsed = stage_end;
            prev_level = *target;
        }

        // Past all stages.
        self.stages.last().map(|(_, l)| *l).unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// LFO
// ---------------------------------------------------------------------------

/// LFO waveform shape.
#[derive(Debug, Clone, Copy)]
pub enum LfoWaveform {
    Sine,
    Triangle,
    Square,
    Sawtooth,
    ReverseSawtooth,
    /// Sample-and-hold with a simple linear congruential generator.
    SampleHold { seed: u64 },
}

/// Low-frequency oscillator used for modulation.
#[derive(Debug, Clone)]
pub struct LfoModulator {
    pub frequency_hz: f64,
    /// Depth of modulation (output range will be [-depth, +depth]).
    pub depth: f64,
    pub waveform: LfoWaveform,
    pub phase_offset: f64,
}

impl LfoModulator {
    /// Compute LFO value at `time_s`. Returns a value in `[-depth, +depth]`.
    pub fn sample_at(&self, time_s: f64) -> f64 {
        let phase = (self.frequency_hz * time_s + self.phase_offset).fract();
        let phase = if phase < 0.0 { phase + 1.0 } else { phase };

        let unit = match self.waveform {
            LfoWaveform::Sine => (2.0 * PI * phase).sin(),
            LfoWaveform::Triangle => {
                if phase < 0.5 {
                    4.0 * phase - 1.0
                } else {
                    3.0 - 4.0 * phase
                }
            }
            LfoWaveform::Square => {
                if phase < 0.5 { 1.0 } else { -1.0 }
            }
            LfoWaveform::Sawtooth => 2.0 * phase - 1.0,
            LfoWaveform::ReverseSawtooth => 1.0 - 2.0 * phase,
            LfoWaveform::SampleHold { seed } => {
                // Simple LCG: advance state based on the integer phase count.
                let step = (phase * 1000.0) as u64;
                let state = seed
                    .wrapping_add(step)
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                // Map to [-1, 1].
                let normalised = (state >> 33) as f64 / (u32::MAX as f64);
                normalised * 2.0 - 1.0
            }
        };

        unit * self.depth
    }
}

// ---------------------------------------------------------------------------
// EnvelopeProcessor
// ---------------------------------------------------------------------------

/// Utility for combining envelope and LFO outputs with audio signals.
pub struct EnvelopeProcessor;

impl EnvelopeProcessor {
    /// Additively modulate an envelope value with an LFO sample.
    pub fn modulate_envelope(env_value: f64, lfo: &LfoModulator, time_s: f64) -> f64 {
        (env_value + lfo.sample_at(time_s)).clamp(0.0, 1.0)
    }

    /// Multiply each sample in `signal` by the ADSR envelope amplitude.
    ///
    /// `gate_off_sample` is the sample index at which the gate was released.
    pub fn apply_to_signal(
        signal: &[f64],
        envelope: &AdsrEnvelope,
        sample_rate: f64,
        gate_off_sample: Option<usize>,
    ) -> Vec<f64> {
        let gate_off_ms = gate_off_sample
            .map(|s| s as f64 / sample_rate * 1000.0);

        signal
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let time_ms = i as f64 / sample_rate * 1000.0;
                let gate_on = gate_off_ms.map(|off| time_ms < off).unwrap_or(true);
                let amp = envelope.sample_at(time_ms, gate_on, gate_off_ms);
                s * amp
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adsr_attack_rises() {
        let env = AdsrEnvelope::new(100.0, 50.0, 0.7, 200.0);
        let a0 = env.sample_at(0.0, true, None);
        let a50 = env.sample_at(50.0, true, None);
        let a100 = env.sample_at(100.0, true, None);
        assert!(a0 <= a50);
        assert!(a50 <= a100);
    }

    #[test]
    fn adsr_sustain_level() {
        let env = AdsrEnvelope::new(10.0, 10.0, 0.6, 50.0);
        let s = env.sample_at(50.0, true, None);
        assert!((s - 0.6).abs() < 1e-9);
    }

    #[test]
    fn adsr_release_decays() {
        let env = AdsrEnvelope::new(10.0, 10.0, 0.8, 100.0);
        let s0 = env.sample_at(100.0, false, Some(100.0));
        let s50 = env.sample_at(150.0, false, Some(100.0));
        let s100 = env.sample_at(200.0, false, Some(100.0));
        assert!(s0 >= s50);
        assert!(s50 >= s100);
    }

    #[test]
    fn apply_curve_bounds() {
        for curve in &[CurveType::Linear, CurveType::Exponential, CurveType::SCurve] {
            assert!((apply_curve(0.0, *curve)).abs() < 1e-9);
            assert!((apply_curve(1.0, *curve) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn multi_stage_interpolation() {
        let mut env = MultiStageEnvelope::new();
        env.add_stage(100.0, 1.0);
        env.add_stage(100.0, 0.0);
        assert!((env.sample_at(0.0)).abs() < 1e-9);
        assert!((env.sample_at(50.0) - 0.5).abs() < 1e-9);
        assert!((env.sample_at(100.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn multi_stage_loop() {
        let mut env = MultiStageEnvelope::new();
        env.add_stage(100.0, 1.0);
        env.add_stage(100.0, 0.0);
        env.set_loop(0);
        // At time = 200ms we should loop back to start of stage 0.
        let v = env.sample_at(200.0);
        assert!((v).abs() < 1e-9);
    }

    #[test]
    fn lfo_sine_range() {
        let lfo = LfoModulator {
            frequency_hz: 1.0,
            depth: 0.5,
            waveform: LfoWaveform::Sine,
            phase_offset: 0.0,
        };
        for i in 0..100 {
            let v = lfo.sample_at(i as f64 * 0.01);
            assert!(v >= -0.5 && v <= 0.5);
        }
    }

    #[test]
    fn envelope_processor_apply_signal() {
        let env = AdsrEnvelope::new(10.0, 10.0, 0.8, 50.0);
        let signal: Vec<f64> = vec![1.0; 100];
        let out = EnvelopeProcessor::apply_to_signal(&signal, &env, 1000.0, Some(80));
        assert_eq!(out.len(), 100);
        // First sample should be near zero (start of attack).
        assert!(out[0] < 0.1);
    }

    #[test]
    fn ahdsr_hold_is_peak() {
        let env = AhdsrEnvelope::new(10.0, 20.0, 30.0, 0.5, 50.0);
        // During hold (10ms – 30ms) level should be 1.0.
        assert!((env.sample_at(20.0, None) - 1.0).abs() < 1e-9);
    }
}
