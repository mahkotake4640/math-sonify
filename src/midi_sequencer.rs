//! MIDI event sequencer.
//!
//! Provides [`MidiSequence`] for scheduling MIDI events on a timeline,
//! and [`Sequencer`] for pattern-based step sequencing.

// ---------------------------------------------------------------------------
// MIDI primitives
// ---------------------------------------------------------------------------

/// A single MIDI note with pitch, velocity, channel, and duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiNote {
    /// MIDI pitch (0–127).
    pub pitch: u8,
    /// Note velocity (0–127).
    pub velocity: u8,
    /// MIDI channel (0–15).
    pub channel: u8,
    /// Duration in ticks.
    pub duration_ticks: u32,
}

/// Individual MIDI events.
#[derive(Debug, Clone, PartialEq)]
pub enum MidiEvent {
    NoteOn {
        pitch: u8,
        velocity: u8,
        channel: u8,
    },
    NoteOff {
        pitch: u8,
        channel: u8,
    },
    ControlChange {
        controller: u8,
        value: u8,
        channel: u8,
    },
    ProgramChange {
        program: u8,
        channel: u8,
    },
    /// Tempo in microseconds per beat.
    Tempo(u32),
    TimeSignature {
        numerator: u8,
        denominator: u8,
    },
}

/// An event placed at an absolute tick position.
#[derive(Debug, Clone)]
pub struct SequenceEvent {
    pub tick: u64,
    pub event: MidiEvent,
}

// ---------------------------------------------------------------------------
// MidiSequence
// ---------------------------------------------------------------------------

/// An ordered list of MIDI events with tick timing.
#[derive(Debug, Clone, Default)]
pub struct MidiSequence {
    pub events: Vec<SequenceEvent>,
    pub ticks_per_beat: u32,
    pub total_ticks: u64,
}

impl MidiSequence {
    /// Create a new empty sequence.
    pub fn new(ticks_per_beat: u32) -> Self {
        Self {
            events: Vec::new(),
            ticks_per_beat,
            total_ticks: 0,
        }
    }

    /// Add a raw event at an absolute tick position.
    pub fn add_event(&mut self, tick: u64, event: MidiEvent) {
        self.events.push(SequenceEvent { tick, event });
        if tick > self.total_ticks {
            self.total_ticks = tick;
        }
        // Keep sorted by tick.
        self.events.sort_by_key(|e| e.tick);
    }

    /// Add a note (NoteOn + NoteOff pair) to the sequence.
    pub fn add_note(&mut self, start_tick: u64, note: MidiNote) {
        self.add_event(
            start_tick,
            MidiEvent::NoteOn {
                pitch: note.pitch,
                velocity: note.velocity,
                channel: note.channel,
            },
        );
        let end_tick = start_tick + note.duration_ticks as u64;
        self.add_event(
            end_tick,
            MidiEvent::NoteOff {
                pitch: note.pitch,
                channel: note.channel,
            },
        );
    }

    /// Snap all event tick positions to the nearest `grid_ticks` boundary.
    pub fn quantize(&mut self, grid_ticks: u32) {
        if grid_ticks == 0 {
            return;
        }
        for event in &mut self.events {
            let grid = grid_ticks as u64;
            event.tick = ((event.tick + grid / 2) / grid) * grid;
        }
        self.events.sort_by_key(|e| e.tick);
        self.total_ticks = self.events.last().map(|e| e.tick).unwrap_or(0);
    }

    /// Transpose all NoteOn/NoteOff pitches by `semitones`.
    pub fn transpose(&mut self, semitones: i32) {
        for ev in &mut self.events {
            match &mut ev.event {
                MidiEvent::NoteOn { pitch, .. } | MidiEvent::NoteOff { pitch, .. } => {
                    let new_pitch = (*pitch as i32 + semitones).clamp(0, 127) as u8;
                    *pitch = new_pitch;
                }
                _ => {}
            }
        }
    }

    /// Scale all tick positions by `factor`.
    pub fn time_stretch(&mut self, factor: f64) {
        for ev in &mut self.events {
            ev.tick = (ev.tick as f64 * factor).round() as u64;
        }
        self.events.sort_by_key(|e| e.tick);
        self.total_ticks = self.events.last().map(|e| e.tick).unwrap_or(0);
    }

    /// Convert tick positions to milliseconds given `tempo_us` (µs per beat).
    pub fn to_absolute_ms(&self, tempo_us: u32) -> Vec<(f64, &MidiEvent)> {
        let us_per_tick = tempo_us as f64 / self.ticks_per_beat as f64;
        self.events
            .iter()
            .map(|e| (e.tick as f64 * us_per_tick / 1000.0, &e.event))
            .collect()
    }

    /// Total sequence duration in milliseconds.
    pub fn duration_ms(&self, tempo_us: u32) -> f64 {
        let us_per_tick = tempo_us as f64 / self.ticks_per_beat as f64;
        self.total_ticks as f64 * us_per_tick / 1000.0
    }
}

// ---------------------------------------------------------------------------
// Pattern
// ---------------------------------------------------------------------------

/// A step-sequencer pattern: a fixed-length list of optional notes.
#[derive(Debug, Clone)]
pub struct Pattern {
    /// One slot per step; `None` means silence.
    pub steps: Vec<Option<MidiNote>>,
    pub length_steps: usize,
    pub loop_count: u32,
}

impl Pattern {
    /// Create a silent pattern of `length_steps` steps.
    pub fn new(length_steps: usize, loop_count: u32) -> Self {
        Self {
            steps: vec![None; length_steps],
            length_steps,
            loop_count,
        }
    }

    /// Set a note at a given step index.
    pub fn set_step(&mut self, index: usize, note: Option<MidiNote>) {
        if index < self.length_steps {
            self.steps[index] = note;
        }
    }
}

// ---------------------------------------------------------------------------
// Sequencer
// ---------------------------------------------------------------------------

/// Pattern-based step sequencer that renders to a [`MidiSequence`].
pub struct Sequencer {
    ticks_per_beat: u32,
    /// Current tempo in BPM.
    bpm: f64,
    /// Registered patterns and their target MIDI channels.
    patterns: Vec<(Pattern, u8)>,
}

impl Sequencer {
    /// Create a new sequencer.
    pub fn new(ticks_per_beat: u32, bpm: f64) -> Self {
        Self {
            ticks_per_beat,
            bpm,
            patterns: Vec::new(),
        }
    }

    /// Register a pattern for the given channel.
    pub fn add_pattern(&mut self, pattern: Pattern, channel: u8) {
        self.patterns.push((pattern, channel));
    }

    /// How many ticks each step occupies for a given step subdivision.
    pub fn ticks_per_step(&self, steps_per_beat: u32) -> u32 {
        if steps_per_beat == 0 {
            return self.ticks_per_beat;
        }
        self.ticks_per_beat / steps_per_beat
    }

    /// Update the tempo.
    pub fn set_tempo(&mut self, bpm: f64) {
        self.bpm = bpm.max(1.0);
    }

    /// Render all patterns for `num_bars` bars into a [`MidiSequence`].
    pub fn render(&self, num_bars: u32) -> MidiSequence {
        let mut seq = MidiSequence::new(self.ticks_per_beat);
        // One bar = 4 beats (4/4).
        let ticks_per_bar = self.ticks_per_beat as u64 * 4;

        for (pattern, channel) in &self.patterns {
            if pattern.length_steps == 0 {
                continue;
            }
            let step_ticks =
                (ticks_per_bar / pattern.length_steps as u64).max(1);

            let total_loops = if pattern.loop_count == 0 {
                num_bars
            } else {
                pattern.loop_count.min(num_bars)
            };

            for bar in 0..total_loops as u64 {
                let bar_offset = bar * ticks_per_bar;
                for (step, maybe_note) in pattern.steps.iter().enumerate() {
                    if let Some(note) = maybe_note {
                        let tick = bar_offset + step as u64 * step_ticks;
                        let mut note = *note;
                        note.channel = *channel;
                        seq.add_note(tick, note);
                    }
                }
            }
        }

        // Insert tempo meta-event at tick 0.
        let tempo_us = (60_000_000.0 / self.bpm) as u32;
        seq.add_event(0, MidiEvent::Tempo(tempo_us));
        seq
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_note(pitch: u8) -> MidiNote {
        MidiNote {
            pitch,
            velocity: 64,
            channel: 0,
            duration_ticks: 120,
        }
    }

    #[test]
    fn add_note_creates_pair() {
        let mut seq = MidiSequence::new(480);
        seq.add_note(0, make_note(60));
        // NoteOn at 0, NoteOff at 120.
        assert_eq!(seq.events.len(), 2);
    }

    #[test]
    fn quantize_snaps_ticks() {
        let mut seq = MidiSequence::new(480);
        seq.add_event(10, MidiEvent::NoteOn { pitch: 60, velocity: 80, channel: 0 });
        seq.quantize(120);
        assert_eq!(seq.events[0].tick, 0);
    }

    #[test]
    fn transpose_shifts_pitch() {
        let mut seq = MidiSequence::new(480);
        seq.add_note(0, make_note(60));
        seq.transpose(12);
        if let MidiEvent::NoteOn { pitch, .. } = &seq.events[0].event {
            assert_eq!(*pitch, 72);
        }
    }

    #[test]
    fn time_stretch_doubles_ticks() {
        let mut seq = MidiSequence::new(480);
        seq.add_note(100, make_note(60));
        seq.time_stretch(2.0);
        // NoteOn was at 100 → 200; NoteOff at 220 → 440.
        let on_tick = seq.events.iter().find(|e| matches!(e.event, MidiEvent::NoteOn { .. })).unwrap().tick;
        assert_eq!(on_tick, 200);
    }

    #[test]
    fn duration_ms_calculation() {
        let mut seq = MidiSequence::new(480);
        seq.add_note(0, make_note(60));
        // At 120 BPM, 500 000 µs/beat, 480 ticks/beat → each tick = 500 000/480 µs.
        let ms = seq.duration_ms(500_000);
        assert!(ms > 0.0);
    }

    #[test]
    fn sequencer_render_produces_events() {
        let mut s = Sequencer::new(480, 120.0);
        let mut pat = Pattern::new(4, 1);
        pat.set_step(0, Some(make_note(60)));
        pat.set_step(2, Some(make_note(64)));
        s.add_pattern(pat, 0);
        let seq = s.render(1);
        assert!(!seq.events.is_empty());
    }

    #[test]
    fn ticks_per_step_correct() {
        let s = Sequencer::new(480, 120.0);
        assert_eq!(s.ticks_per_step(4), 120);
    }
}
