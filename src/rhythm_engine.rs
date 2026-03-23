//! Polyrhythm, Euclidean rhythms, and swing quantization.
//!
//! Implements the Bjorklund algorithm for Euclidean rhythm generation,
//! swing quantization, polyrhythm layering, and groove templates.

// ---------------------------------------------------------------------------
// Beat and Pattern
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Beat {
    pub position: u32,
    pub velocity: u8,
    pub duration: u32,
    pub is_accent: bool,
}

#[derive(Debug, Clone)]
pub struct Pattern {
    pub beats: Vec<Beat>,
    pub length_ticks: u32,
    pub time_sig: (u8, u8),
}

// ---------------------------------------------------------------------------
// Euclidean rhythm — Bjorklund algorithm
// ---------------------------------------------------------------------------

/// Distribute `pulses` evenly across `steps` using the Bjorklund algorithm.
pub fn euclidean_rhythm(pulses: u32, steps: u32) -> Vec<bool> {
    if steps == 0 {
        return Vec::new();
    }
    if pulses == 0 {
        return vec![false; steps as usize];
    }
    if pulses >= steps {
        return vec![true; steps as usize];
    }

    // Bresenham-style distribution
    let mut pattern = vec![false; steps as usize];
    let mut bucket = 0i32;
    for i in 0..steps as usize {
        bucket += pulses as i32;
        if bucket >= steps as i32 {
            bucket -= steps as i32;
            pattern[i] = true;
        }
    }
    pattern
}

impl Pattern {
    /// Build a pattern from an Euclidean rhythm.
    pub fn from_euclidean(pulses: u32, steps: u32, step_ticks: u32, velocity: u8) -> Self {
        let rhythm = euclidean_rhythm(pulses, steps);
        let length_ticks = steps * step_ticks;
        let beats = rhythm
            .iter()
            .enumerate()
            .filter_map(|(i, &hit)| {
                if hit {
                    Some(Beat {
                        position: i as u32 * step_ticks,
                        velocity,
                        duration: step_ticks,
                        is_accent: i == 0, // accent on the first beat
                    })
                } else {
                    None
                }
            })
            .collect();
        Self {
            beats,
            length_ticks,
            time_sig: (4, 4),
        }
    }
}

// ---------------------------------------------------------------------------
// Swing quantization
// ---------------------------------------------------------------------------

/// Shift off-beat hits by `swing_pct` * beat_duration.
/// swing_pct = 0.5 → straight, 0.67 → triplet swing.
pub fn swing_quantize(beats: &[Beat], swing_pct: f64, resolution_ticks: u32) -> Vec<Beat> {
    let half = resolution_ticks / 2;
    beats
        .iter()
        .map(|beat| {
            let mut b = beat.clone();
            // Determine if this beat falls on an "off-beat" subdivision
            let pos_in_beat = beat.position % resolution_ticks;
            if pos_in_beat == half {
                // It's on the off-beat — apply swing shift
                let shift = (swing_pct * resolution_ticks as f64) as u32;
                let beat_num = beat.position / resolution_ticks;
                b.position = beat_num * resolution_ticks + shift;
            }
            b
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Polyrhythm
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PolyrhythmLayer {
    pub pattern: Pattern,
    pub instrument: String,
    pub pitch: u8,
}

#[derive(Debug, Clone)]
pub struct Polyrhythm {
    pub layers: Vec<PolyrhythmLayer>,
    pub global_tempo_bpm: u32,
}

impl Polyrhythm {
    pub fn new(tempo_bpm: u32) -> Self {
        Self {
            layers: Vec::new(),
            global_tempo_bpm: tempo_bpm,
        }
    }

    pub fn add_layer(mut self, pattern: Pattern, instrument: &str, pitch: u8) -> Self {
        self.layers.push(PolyrhythmLayer {
            pattern,
            instrument: instrument.to_string(),
            pitch,
        });
        self
    }

    /// Compute the LCM of all layer pattern lengths.
    pub fn lcm_length(&self) -> u32 {
        self.layers
            .iter()
            .map(|l| l.pattern.length_ticks)
            .fold(1u32, |acc, len| lcm(acc, len))
    }

    /// Render `num_cycles` repetitions. Returns (tick, pitch, velocity) tuples sorted by tick.
    pub fn render(&self, num_cycles: u32) -> Vec<(u32, u8, u8)> {
        let cycle_len = self.lcm_length();
        let total_ticks = cycle_len * num_cycles;
        let mut events: Vec<(u32, u8, u8)> = Vec::new();

        for layer in &self.layers {
            let pattern_len = layer.pattern.length_ticks;
            if pattern_len == 0 {
                continue;
            }
            let mut tick = 0u32;
            while tick < total_ticks {
                for beat in &layer.pattern.beats {
                    let abs_tick = tick + beat.position;
                    if abs_tick < total_ticks {
                        events.push((abs_tick, layer.pitch, beat.velocity));
                    }
                }
                tick += pattern_len;
            }
        }

        events.sort_unstable_by_key(|(t, _, _)| *t);
        events
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn lcm(a: u32, b: u32) -> u32 {
    if a == 0 || b == 0 {
        0
    } else {
        a / gcd(a, b) * b
    }
}

// ---------------------------------------------------------------------------
// Groove templates
// ---------------------------------------------------------------------------

const STEP_TICKS: u32 = 120; // 16th note = 120 ticks at 480 PPQN

/// Return a named groove pattern.
pub fn groove_template(name: &str) -> Pattern {
    match name {
        "bossa" => {
            // 2-3 clave on 16 steps
            // Hits on steps: 0, 3, 6, 10, 13 (0-indexed)
            let hits = [0usize, 3, 6, 10, 13];
            let steps = 16u32;
            let length_ticks = steps * STEP_TICKS;
            let beats = hits
                .iter()
                .map(|&s| Beat {
                    position: s as u32 * STEP_TICKS,
                    velocity: if s == 0 { 110 } else { 80 },
                    duration: STEP_TICKS,
                    is_accent: s == 0,
                })
                .collect();
            Pattern { beats, length_ticks, time_sig: (4, 4) }
        }
        "samba" => {
            // Surdo pattern: beats 1 and 3 (16 step grid)
            let hits = [0usize, 8];
            let steps = 16u32;
            let length_ticks = steps * STEP_TICKS;
            let beats = hits
                .iter()
                .map(|&s| Beat {
                    position: s as u32 * STEP_TICKS,
                    velocity: 100,
                    duration: STEP_TICKS * 2,
                    is_accent: s == 0,
                })
                .collect();
            Pattern { beats, length_ticks, time_sig: (4, 4) }
        }
        "jazz" => {
            // Ride cymbal: quarter notes with off-beat 8ths
            // Hits on 0, 4, 6, 8, 12 (16th-note grid)
            let hits = [0usize, 4, 6, 8, 12];
            let steps = 16u32;
            let length_ticks = steps * STEP_TICKS;
            let beats = hits
                .iter()
                .map(|&s| Beat {
                    position: s as u32 * STEP_TICKS,
                    velocity: if s % 8 == 0 { 100 } else { 75 },
                    duration: STEP_TICKS,
                    is_accent: s % 8 == 0,
                })
                .collect();
            Pattern { beats, length_ticks, time_sig: (4, 4) }
        }
        "funk" => {
            // 16th note hi-hat with accents on beat 1, 5, 9, 13
            let steps = 16u32;
            let length_ticks = steps * STEP_TICKS;
            let beats = (0..steps)
                .map(|s| Beat {
                    position: s * STEP_TICKS,
                    velocity: if s % 4 == 0 { 110 } else { 60 },
                    duration: STEP_TICKS,
                    is_accent: s % 4 == 0,
                })
                .collect();
            Pattern { beats, length_ticks, time_sig: (4, 4) }
        }
        _ => {
            // "straight" — 4-on-floor kick pattern
            let hits = [0usize, 4, 8, 12];
            let steps = 16u32;
            let length_ticks = steps * STEP_TICKS;
            let beats = hits
                .iter()
                .map(|&s| Beat {
                    position: s as u32 * STEP_TICKS,
                    velocity: 100,
                    duration: STEP_TICKS,
                    is_accent: true,
                })
                .collect();
            Pattern { beats, length_ticks, time_sig: (4, 4) }
        }
    }
}

// ---------------------------------------------------------------------------
// Humanize
// ---------------------------------------------------------------------------

/// Apply timing and velocity jitter using an LCG PRNG seeded with `seed`.
pub fn humanize(
    beats: &[Beat],
    timing_variance: f64,
    velocity_variance: u8,
    seed: u64,
) -> Vec<Beat> {
    let mut state = seed;
    let mut lcg = move || -> u64 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        state
    };

    beats
        .iter()
        .map(|beat| {
            let timing_jitter = ((lcg() % 1000) as f64 / 1000.0 - 0.5) * 2.0 * timing_variance;
            let vel_jitter = (lcg() % (velocity_variance as u64 * 2 + 1)) as i32
                - velocity_variance as i32;

            let new_position = (beat.position as f64 + timing_jitter).max(0.0) as u32;
            let new_velocity = (beat.velocity as i32 + vel_jitter).clamp(1, 127) as u8;

            Beat {
                position: new_position,
                velocity: new_velocity,
                duration: beat.duration,
                is_accent: beat.is_accent,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euclidean_5_in_8() {
        let pattern = euclidean_rhythm(5, 8);
        assert_eq!(pattern.len(), 8);
        let hits: u32 = pattern.iter().map(|&b| b as u32).sum();
        assert_eq!(hits, 5);
    }

    #[test]
    fn test_euclidean_zero_pulses() {
        let pattern = euclidean_rhythm(0, 8);
        assert_eq!(pattern.len(), 8);
        assert!(pattern.iter().all(|&b| !b));
    }

    #[test]
    fn test_swing_shifts_off_beats() {
        let beats = vec![
            Beat { position: 0,   velocity: 100, duration: 240, is_accent: true },
            Beat { position: 240, velocity: 80,  duration: 240, is_accent: false },
            Beat { position: 480, velocity: 100, duration: 240, is_accent: true },
            Beat { position: 720, velocity: 80,  duration: 240, is_accent: false },
        ];
        let swung = swing_quantize(&beats, 0.67, 480);
        // Off-beats (240, 720) should be shifted
        assert_ne!(swung[1].position, beats[1].position);
        assert_ne!(swung[3].position, beats[3].position);
        // On-beats (0, 480) should remain unchanged
        assert_eq!(swung[0].position, beats[0].position);
        assert_eq!(swung[2].position, beats[2].position);
    }

    #[test]
    fn test_lcm_of_3_and_4_is_12() {
        let p3 = Pattern::from_euclidean(3, 3, 480, 100);
        let p4 = Pattern::from_euclidean(4, 4, 360, 100);
        let poly = Polyrhythm::new(120)
            .add_layer(p3, "drum", 36)
            .add_layer(p4, "hihat", 42);
        assert_eq!(poly.lcm_length(), 1440);
    }

    #[test]
    fn test_groove_template_returns_non_empty() {
        for name in &["bossa", "samba", "jazz", "funk", "straight"] {
            let p = groove_template(name);
            assert!(!p.beats.is_empty(), "Groove '{}' should have beats", name);
        }
    }

    #[test]
    fn test_humanize_changes_values() {
        let beats = vec![
            Beat { position: 0,   velocity: 100, duration: 480, is_accent: true },
            Beat { position: 480, velocity: 80,  duration: 480, is_accent: false },
            Beat { position: 960, velocity: 90,  duration: 480, is_accent: false },
        ];
        let humanized = humanize(&beats, 20.0, 10, 42);
        // At least some values should differ from the originals
        let any_different = beats.iter().zip(humanized.iter()).any(|(orig, h)| {
            orig.position != h.position || orig.velocity != h.velocity
        });
        assert!(any_different, "humanize should change at least some values");
    }

    #[test]
    fn test_polyrhythm_render_sorted() {
        let p3 = Pattern::from_euclidean(3, 3, 480, 100);
        let p4 = Pattern::from_euclidean(4, 4, 360, 80);
        let poly = Polyrhythm::new(120)
            .add_layer(p3, "bass", 36)
            .add_layer(p4, "snare", 38);
        let events = poly.render(2);
        for w in events.windows(2) {
            assert!(w[0].0 <= w[1].0, "Events should be sorted by tick");
        }
    }
}
