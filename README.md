# Math Sonify

> **Real-time procedural audio from mathematical dynamical systems.**
> The math drives the music. You hear chaos, synchronization, and orbital resonance translated directly into sound — and now as a DAW plugin.

---

## What is this?

Math Sonify runs continuous simulations of strange attractors, coupled oscillators, and gravitational systems, then maps their evolving state to polyphonic audio in real time. It's not a preset synth with math-themed names. The differential equations are actually running — what you hear *is* the math.

Crank the coupling on the Kuramoto oscillators and hear synchronization emerge. Drop the Lorenz system into a chaotic regime and hear the butterfly spiral through pitch space. Stack a Duffing arpeggio over a Torus drone over a Kuramoto bass — three attractors running simultaneously, each modulating the others' timbral space through the shared effects chain.

The strangest sounds happen during morphs — when the arranger is transitioning between scenes and two attractor geometries are simultaneously deforming into each other. Those sounds exist for 15 seconds and then are gone forever. That's the music.

---

## Get Started

### Standalone app (Windows, no install)

1. Download `math-sonify.exe` from the [latest release](https://github.com/Mattbusel/math-sonify/releases/latest)
2. Double-click — audio starts immediately (Lorenz attractor, pentatonic scale)

> Windows SmartScreen may warn on first run. Click **More info → Run anyway**.

### VST3 / CLAP Plugin (DAW integration)

1. Download `MathSonify.vst3.zip` from the [latest release](https://github.com/Mattbusel/math-sonify/releases/latest)
2. Extract `MathSonify.vst3` to `C:\Program Files\Common Files\VST3\`
3. Rescan plugins in your DAW (Ableton, FL Studio, Reaper, etc.)
4. Load **Math Sonify** as an instrument track — it receives MIDI note-on/off for pitch control

### Build from source

Requires [Rust](https://rustup.rs/) stable 1.75+.

```bash
git clone https://github.com/Mattbusel/math-sonify
cd math-sonify
cargo run --release          # standalone app
cargo build --release --lib  # plugin .dll
```

---

## Dynamical Systems

| System | Dimension | Integrator | Character |
|--------|-----------|------------|-----------|
| **Lorenz** | 3D | RK4 | Chaotic butterfly — sensitive to σ/ρ/β |
| **Rössler** | 3D | RK4 | Slower spiral chaos, more melodic |
| **Double Pendulum** | 4D | Symplectic leapfrog | Chaotic with quasi-periodic musical regimes |
| **Geodesic Torus** | 4D | RK4 | Irrational winding → ergodic drone |
| **Kuramoto** | 8D | RK4 | Synchronization transition: noise → harmony |
| **Three-Body** | 12D | Symplectic | Figure-8 orbit, gravitational rhythm |
| **Duffing** | 2D | RK4 | Period-doubling route to chaos, rhythmic clicking |
| **Van der Pol** | 2D | RK4 | Limit cycle relaxation oscillator |
| **Halvorsen** | 3D | RK4 | Dense spiral attractor, layered harmonic drifts |
| **Aizawa** | 3D | RK4 | Toroidal attractor with slow wobble |
| **Chua** | 3D | RK4 | Double-scroll — raw electronic buzz |

---

## Sonification Modes

| Mode | How it works |
|------|-------------|
| **Direct** | State variables → quantized oscillator frequencies on a musical scale |
| **Orbital** | Instantaneous angular velocity = fundamental; Lyapunov exponent → inharmonicity |
| **Granular** | Trajectory speed → grain density; position → grain pitch |
| **Spectral** | State vector → 32-partial additive spectral envelope |
| **FM** | Attractor drives carrier/modulator ratio and index — frequency modulation synthesis |

---

## Preset Library (58 patches)

Grouped by genre/character. All automatable in the plugin.

| Category | Examples |
|----------|---------|
| **Ambient / Space** | Lorenz Ambience, Torus Drone, Aizawa Nebula, Halvorsen Drift |
| **Techno / Industrial** | Duffing Techno Kick, Chua Industrial Grind, VDP Dark Pulse |
| **Jazz / Harmonic** | Three-Body Jazz, Kuramoto Jazz Sync, Aizawa Quartet |
| **Glitch / Experimental** | FM Chaos, Chua Grit, Duffing Glitch Arp |
| **Cinematic / Orchestral** | Torus Cinema, Lorenz Cinematic, Three-Body Overture |
| **Bass / Sub** | Lorenz Sub Bass, Duffing Bass Hit, Chua Grit Bass |
| **Lead / Melody** | Rössler Lead, Aizawa Lead |
| **Drone / Meditation** | Torus Drone, Pendulum Meditation, VDP Meditation |
| **Arpeggio** | Lorenz Pentatonic Arp, Duffing Arp, Kuramoto Arp |
| **Hybrid** | Torus Spectral Warp, Lorenz FM Drone, Three-Body Pulse |

---

## Multi-Layer Polyphony

Run up to **3 independent attractor systems simultaneously** from the MIXER tab:

- **Layer 0** — your main system, controlled by the left panel
- **Layer 1 / Layer 2** — each picks any of the 58 presets, with independent level, pan, and mute
- All 3 layers feed into one shared reverb/delay/chorus/limiter master chain
- Each layer has its own **velocity-sensitive ADSR** envelope (attack/decay/sustain/release)

```
Layer 0: Lorenz pad (slow, reverb-heavy)
Layer 1: Duffing arpeggio (rhythmic, dry)  →  Shared effects chain  →  Output
Layer 2: Kuramoto bass (low, synchronized)
```

---

## ADSR Envelopes

Every layer has 4 independent ADSR envelopes (one per voice), triggered by arpeggiator steps and Karplus-Strong events:

- **Velocity-sensitive**: harder MIDI velocity / louder attractor hit = faster attack, longer release
- **Attractor-mapped**: attack time shortens as attractor speed increases — dynamic response is driven by the math
- Controls: Attack (1ms–2s), Decay (1ms–2s), Sustain (0–1), Release (10ms–5s), all logarithmic

---

## Scene Arranger + Generate Song

Build arrangements of up to 8 scenes in the **ARRANGE tab**:

- Each scene stores a full system + effects snapshot
- Scenes **morph** between each other using smooth parameter interpolation across all fields simultaneously
- **The morph IS the music** — the transition period where two attractors are deforming into each other produces sounds that exist only in that moment

**Generate Song** (one click):
- Pick a mood — 🌙 Ambient, ⚡ Rhythmic, or 🔬 Experimental
- Click 🎲 Generate — fills all 8 scenes with a seeded arrangement
- Hold times: 15–22 seconds. Morph times: 25–35 seconds (by design — you spend most of the time in transition)

---

## MIXER Tab

- **VU meters** for all 3 layers + master bus (green → orange → red at 70% / 90% headroom)
- Per-layer level, pan, mute
- ADSR controls per layer
- **Audio sidechain input** — enable mic/line-in and route it to modulate any parameter (speed, reverb wet, filter cutoff, Lorenz σ, master volume)

---

## Save Clip (Share Button)

**📸 Save Clip** in the MIXER tab captures:

1. The last **60 seconds of audio** as a 32-bit float stereo WAV
2. A **512×512 phase portrait PNG** of the current attractor trail

Both files are timestamped and saved to a `clips/` folder next to the executable. One click — the moment is preserved and ready to post.

---

## VST3 / CLAP Plugin Details

The plugin exposes 17 automatable parameters:

| Parameter | Range | Description |
|-----------|-------|-------------|
| Master Volume | 0–1 | Output level |
| Reverb Wet | 0–1 | Freeverb wet mix |
| Delay Time | 1–2000 ms | Delay line time |
| Delay Feedback | 0–0.9 | Delay feedback |
| Lorenz σ | 1–30 | Sigma (divergence) |
| Lorenz ρ | 10–60 | Rho (chaos threshold near 28) |
| Lorenz β | 0.5–8 | Beta |
| Speed | 0.05–10 | Attractor integration speed |
| Base Frequency | 20–1000 Hz | Root pitch |
| Octave Range | 0.5–6 | Pitch mapping range |
| Chorus Mix | 0–1 | Chorus wet |
| Drive | 1–10 | Waveshaper distortion |
| Portamento | 1–2000 ms | Frequency glide time |
| Attack | 1–2000 ms | ADSR attack |
| Decay | 1–2000 ms | ADSR decay |
| Sustain | 0–1 | ADSR sustain level |
| Release | 10–5000 ms | ADSR release |

MIDI: note-on sets pitch (overrides attractor pitch) and triggers ADSR. Note-off releases the envelope.

---

## Architecture

```
Simulation thread (120 Hz)
─────────────────────────
Layer 0: DynamicalSystem::step()  ─┐
Layer 1: DynamicalSystem::step()  ─┤──▶  crossbeam channel  ──▶  Audio thread (44100/48000 Hz)
Layer 2: DynamicalSystem::step()  ─┘                               ├─ LayerSynth[0]: 4 osc + 4 ADSR + KS
                                                                    ├─ LayerSynth[1]: 4 osc + 4 ADSR + KS
Sonification::map()                                                 ├─ LayerSynth[2]: 4 osc + 4 ADSR + KS
  └─ state → AudioParams per layer                                  ├─ Σ sum → shared master FX chain
                                                                    │   ├─ BiquadFilter
UI thread (30 Hz)                                                   │   ├─ DelayLine
─────────────                                                       │   ├─ Chorus
egui panels                                                         │   ├─ Freeverb
  ├─ Phase portrait, Waveform,                                      │   └─ Lookahead limiter
  ├─ Note Map, ARRANGE                                              └─ Clip ring buffer (60s)
  ├─ MIXER (VU, ADSR, layers)                                           ├─ WAV export
  └─ Math View, Bifurcation                                             └─ PNG portrait export
```

- Lock-free audio callback — no allocations, no blocking mutex in the hot path
- All sim→audio communication via `crossbeam_channel::try_send` (drops on backpressure)
- Clip buffer and waveform capture via `parking_lot::Mutex::try_lock()` (skips on contention)

---

## Arpeggiator

16th-note step sequencer (2–32 steps) driven by BPM:

- Step pitch quantized to the selected musical scale, with the current attractor state modulating ±5% per step
- Each step triggers the ADSR envelope on all voices + a Karplus-Strong plucked string
- Velocity tracks attractor trajectory speed — fast chaotic phases hit harder

---

## Effects Chain

Per layer: **Waveshaper** (tanh saturation) → **Bitcrusher** (bit depth + sample rate reduction)

Master bus: **Biquad LP filter** → **Delay line** (BPM-syncable) → **3-voice chorus** → **Freeverb** → **Lookahead limiter** → NaN guard

---

## Musical Tips

**Ambient pad:** Load *Torus Drone* → reverb 0.65 → portamento 400ms → Major chord. The irrational geodesic winding produces slow consonant drifts that never repeat.

**Hear synchronization happen:** Load *Kuramoto Sync* → coupling K = 0.5 (incoherent noise) → slowly drag K to 3.0 → listen as tones lock into harmony. That's the Kuramoto phase transition in real time.

**Chaotic rhythm:** Load *Duffing Rhythm* → Granular mode → speed 3.0. The period-doubling route to chaos produces clicking rhythms that gradually destabilize.

**Polyphonic layering:** In the MIXER tab, set Layer 1 to *Torus Drone* (pad, reverb 0.8) and Layer 2 to *Duffing Rhythm* (rhythm, dry). The torus provides a harmonic foundation while the Duffing drives the rhythm — two completely different mathematical objects, one mix.

**Morph arrangement:** Use Generate Song (Rhythmic mood) → hit Play in the ARRANGE tab → wait for the first morph to begin. The 25-second transition between Duffing and Kuramoto is where the actual music lives.

---

## Built with

- [Rust](https://www.rust-lang.org/)
- [cpal](https://github.com/RustAudio/cpal) — cross-platform audio I/O
- [egui](https://github.com/emilk/egui) / [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) — immediate-mode GUI
- [nih-plug](https://github.com/robbert-vdh/nih-plug) — VST3 / CLAP plugin framework
- [crossbeam-channel](https://github.com/crossbeam-rs/crossbeam) — lock-free inter-thread communication
- [parking_lot](https://github.com/Amanieu/parking_lot) — fast mutexes
- [hound](https://github.com/ruuda/hound) — WAV recording
- [png](https://github.com/image-rs/image-png) — phase portrait export
- [serde](https://serde.rs/) + [toml](https://docs.rs/toml) — config

---

## License

MIT

---

`#rust` `#audio` `#dsp` `#generativemusic` `#chaostheory` `#dynamicalsystems` `#sounddesign` `#mathmusic` `#lorenz` `#kuramoto` `#strangeattractors` `#vst3` `#clap` `#plugin` `#ambientmusic` `#creativecoding` `#procedural`
