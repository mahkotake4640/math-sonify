/// ADSR envelope generator (per-sample).
#[derive(Clone, Copy, PartialEq)]
enum Stage { Idle, Attack, Decay, Sustain, Release }

pub struct Adsr {
    stage: Stage,
    level: f32,
    attack_rate: f32,
    decay_rate: f32,
    sustain_level: f32,
    release_rate: f32,
}

impl Adsr {
    pub fn new(attack_ms: f32, decay_ms: f32, sustain: f32, release_ms: f32, sample_rate: f32) -> Self {
        let ms_to_rate = |ms: f32| 1.0 / (ms * 0.001 * sample_rate).max(1.0);
        Self {
            stage: Stage::Idle,
            level: 0.0,
            attack_rate: ms_to_rate(attack_ms),
            decay_rate: ms_to_rate(decay_ms),
            sustain_level: sustain.clamp(0.0, 1.0),
            release_rate: ms_to_rate(release_ms),
        }
    }

    pub fn trigger(&mut self) { self.stage = Stage::Attack; }
    pub fn release(&mut self) { self.stage = Stage::Release; }
    pub fn is_idle(&self) -> bool { self.stage == Stage::Idle }

    pub fn next_sample(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => self.level = 0.0,
            Stage::Attack => {
                self.level += self.attack_rate;
                if self.level >= 1.0 { self.level = 1.0; self.stage = Stage::Decay; }
            }
            Stage::Decay => {
                self.level -= self.decay_rate;
                if self.level <= self.sustain_level {
                    self.level = self.sustain_level;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {} // hold until release()
            Stage::Release => {
                self.level -= self.release_rate;
                if self.level <= 0.0 { self.level = 0.0; self.stage = Stage::Idle; }
            }
        }
        self.level
    }
}
