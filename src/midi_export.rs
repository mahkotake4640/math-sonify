//! MIDI export: record parameter values and export as a MIDI file.
//!
//! Records a time series of pitch/velocity values derived from the attractor
//! state, then quantises them to standard MIDI pitch values and writes a
//! Type-0 MIDI file using the `midly` crate.

use midly::{
    Format, Header, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
    num::{u4, u7, u15, u24, u28},
    MetaMessage,
};

/// A single recorded parameter frame.
#[derive(Clone, Debug)]
pub struct MidiFrame {
    /// MIDI pitch (0–127).
    pub pitch: u8,
    /// MIDI velocity (1–127).
    pub velocity: u8,
    /// Duration in ticks (one tick per control-rate step in the recording).
    pub ticks: u32,
}

/// Convert attractor coordinates to MIDI pitch + velocity.
///
/// `x_norm`: normalised x in [-1, 1]
/// `y_norm`: normalised y magnitude in [0, 1]
/// `base_midi`: MIDI note of the lowest note (e.g. 48 = C3)
/// `semitone_range`: number of semitones spanning the octave range
/// `scale_offsets`: semitone offsets of the scale degrees
pub fn coords_to_midi(
    x_norm: f64,
    y_norm: f64,
    base_midi: u8,
    semitone_range: u8,
    scale_offsets: &[i32],
) -> (u8, u8) {
    // Map x_norm [-1,1] to [0, semitone_range]
    let t = ((x_norm + 1.0) * 0.5).clamp(0.0, 1.0);
    let semitone_float = t * semitone_range as f64;
    let octave = (semitone_float / 12.0) as i32;
    let semitone_in_octave = (semitone_float as i32) % 12;
    // Find closest scale offset
    let closest_offset = scale_offsets
        .iter()
        .min_by_key(|&&o| (o - semitone_in_octave).abs())
        .copied()
        .unwrap_or(0);
    let raw_pitch = base_midi as i32 + octave * 12 + closest_offset;
    let pitch = raw_pitch.clamp(0, 127) as u8;
    let vel = (20.0 + y_norm * 107.0).clamp(1.0, 127.0) as u8;
    (pitch, vel)
}

/// Scale degree offsets for common scales.
pub fn scale_offsets(scale: &str) -> Vec<i32> {
    match scale {
        "pentatonic" => vec![0, 2, 4, 7, 9],
        "major" => vec![0, 2, 4, 5, 7, 9, 11],
        "minor" => vec![0, 2, 3, 5, 7, 8, 10],
        "chromatic" => (0..12).collect(),
        "blues" => vec![0, 3, 5, 6, 7, 10],
        "whole_tone" => vec![0, 2, 4, 6, 8, 10],
        "diminished" => vec![0, 2, 3, 5, 6, 8, 9, 11],
        _ => vec![0, 2, 4, 7, 9], // default pentatonic
    }
}

/// Write the recorded frames to a MIDI file at the given path.
///
/// Produces a Type-0 single-track MIDI file at 120 BPM by default.
/// `ticks_per_beat` controls the time resolution; at 60 ticks/beat and
/// 120 BPM, 1 tick = 1 control-rate frame (1/120 s).
pub fn export_midi(
    frames: &[MidiFrame],
    path: &str,
    ticks_per_beat: u16,
    tempo_us: u32, // microseconds per beat (500000 = 120 BPM)
    channel: u4,
) -> anyhow::Result<()> {
    if frames.is_empty() {
        anyhow::bail!("No MIDI frames to export");
    }

    let mut events: Vec<TrackEvent<'static>> = Vec::new();

    // Tempo meta event at tick 0
    events.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(tempo_us))),
    });

    let mut abs_tick: u32 = 0;
    let mut prev_abs: u32 = 0;

    for frame in frames {
        let dur = frame.ticks.max(1);
        let note = u7::new(frame.pitch.clamp(0, 127));
        let vel = u7::new(frame.velocity.clamp(1, 127));

        // Note On
        let delta_on = abs_tick.saturating_sub(prev_abs);
        events.push(TrackEvent {
            delta: u28::new(delta_on),
            kind: TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOn { key: note, vel },
            },
        });
        prev_abs = abs_tick;
        abs_tick += dur;

        // Note Off
        let delta_off = abs_tick.saturating_sub(prev_abs);
        events.push(TrackEvent {
            delta: u28::new(delta_off),
            kind: TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOff { key: note, vel: u7::new(0) },
            },
        });
        prev_abs = abs_tick;
    }

    // End of track
    events.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });

    let smf = Smf {
        header: Header {
            format: Format::SingleTrack,
            timing: Timing::Metrical(u15::new(ticks_per_beat)),
        },
        tracks: vec![events],
    };

    let mut buf = Vec::new();
    smf.write_std(&mut buf).map_err(|e| anyhow::anyhow!("MIDI write error: {e}"))?;
    std::fs::write(path, &buf).map_err(|e| anyhow::anyhow!("File write error: {e}"))?;
    Ok(())
}
