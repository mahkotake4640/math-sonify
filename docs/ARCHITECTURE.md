# Architecture

## Three-thread model

```
┌──────────────────────────────────────────────────────────────────┐
│  Simulation thread  (120 Hz)                                     │
│                                                                  │
│  DynamicalSystem::step() → Sonification::map() → AudioParams    │
│                                     │                            │
│              crossbeam bounded channel  (cap = 16)              │
│                   try_send — drops on backpressure               │
│                                     │                            │
│                                     ▼                            │
│  Audio thread  (44 100 Hz, cpal callback)                        │
│                                                                  │
│  recv latest AudioParams → LayerSynth DSP → effects → output    │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│  UI thread  (~30 Hz, eframe/egui, main thread)                   │
│                                                                  │
│  read AppState (parking_lot::Mutex, try_lock) → draw_ui()       │
│  write user input back into AppState                             │
└──────────────────────────────────────────────────────────────────┘
```

The three threads never share a direct reference to each other. The sim→audio path uses a lock-free channel; the sim↔UI path uses a mutex with `try_lock` so neither thread ever blocks waiting for the other.

---

## Thread responsibilities

### Simulation thread (120 Hz)

- Owns the `Box<dyn DynamicalSystem>` and advances it with `step(dt)` once per tick.
- Calls the active `Sonification` mapper to produce an `AudioParams` struct from the current state.
- Sends `[Option<AudioParams>; 3]` (one slot per polyphony layer) over the bounded crossbeam channel with `try_send`. If the channel is full the sample is silently dropped — the audio thread always plays the last received value, so a skipped sim tick causes no audible glitch.
- Reads `AppState` (under `parking_lot::Mutex::try_lock`) to pick up user-changed parameters: system selection, config, sonification mode, layer settings.
- Appends to the shared `viz_history` ring buffer for the phase-portrait display.
- Runs background analytics (Lyapunov spectrum, Poincaré section, bifurcation scans) at reduced cadence without blocking the hot path.

### Audio thread (44 100 Hz, cpal callback)

- Runs entirely inside the `cpal` audio callback — must never block or allocate on the heap.
- Receives the latest `AudioParams` from the channel (`try_recv`; holds the previous value if nothing new arrived).
- Dispatches to the appropriate synthesis path based on `AudioParams::mode` (`Direct`, `Orbital`, `Granular`, `Spectral`, `FM`, `Vocal`, `Waveguide`).
- Up to three independent `LayerSynth` instances are mixed into a shared effects chain: FDN reverb → delay → limiter → master volume.
- Writes peak levels to the shared `VuMeter` and appends interleaved stereo samples to the `ClipBuffer`.
- Writes to `WavRecorder` (WAV file capture) when recording is active.

### UI thread (~30 Hz, main thread)

- Runs on the main OS thread as required by eframe/egui.
- Reads `AppState` and `viz_history` under `try_lock` to render phase portraits, waveforms, VU meters, and parameter controls.
- User interactions write back into `AppState`; the sim thread picks them up on the next tick.
- Requests repaints at ~30 Hz (`request_repaint_after(33 ms)`).

---

## Key data structures

### `AudioParams` (`src/sonification/mod.rs`)

The message type sent from sim to audio. A plain `Clone` struct (no heap allocation) containing:

- `freqs: [f32; 4]` — oscillator frequencies for up to four voices
- `amps: [f32; 4]` — per-voice amplitudes
- `pans: [f32; 4]` — stereo pan positions
- `mode: SonifMode` — which synthesis path to use
- `partials: [f32; 32]` — harmonic amplitudes for spectral mode
- `grain_spawn_rate`, `grain_base_freq`, `grain_freq_spread` — granular mode
- `fm_carrier_freq`, `fm_mod_ratio`, `fm_mod_index` — FM mode
- Effects parameters: `reverb_wet`, `delay_ms`, `delay_feedback`, `chorus_mix`, `waveshaper_drive`, `bit_depth`, `rate_crush`, `eq_*`
- ADSR envelope times and sustain level
- `layer_id`, `layer_level`, `layer_pan` — polyphony layer identity
- `chaos_level` — estimated chaoticity forwarded to the UI

### `ClipBuffer` (`src/audio.rs`)

```rust
pub type ClipBuffer = Arc<Mutex<VecDeque<f32>>>;
```

A circular buffer of the last ~60 seconds of stereo interleaved `f32` output, capped at `CLIP_SECONDS * sample_rate * 2` samples. Written by the audio thread, read by the UI for waveform display and by the export path for loop/clip saving.

### `VuMeter` (`src/audio.rs`)

```rust
pub type VuMeter = Arc<Mutex<[f32; 4]>>;
```

Four peak-hold values: `[layer0_peak, layer1_peak, layer2_peak, master_peak]`. Updated each audio callback, read by the UI mixer panel.

### `AppState` (`src/ui.rs`)

The central shared state between the sim and UI threads, protected by a `parking_lot::Mutex`. Holds:

- Current `Config` (system parameters, sonification config, audio config)
- Active system name and layer configurations
- UI-side state: selected preset, display mode, recording flags, arrangement cursor
- References to shared handles: `VuMeter`, `ClipBuffer`, `SidechainLevel`, `SharedSnippetPlayback`

---

## Concurrency pattern

The design is **lock-free on the hot audio path** and **try-lock everywhere else**:

| Path | Mechanism |
|------|-----------|
| Sim → Audio (params) | `crossbeam::bounded` channel, `try_send` / `try_recv` |
| Sim ↔ UI (state) | `parking_lot::Mutex`, `try_lock` (non-blocking) |
| Audio → UI (VU, clip) | `parking_lot::Mutex`, `try_lock` in UI read path |
| UI → Sim (config changes) | Write into `AppState` under mutex; sim reads next tick |

**Backpressure on the sim→audio channel**: the channel capacity is 16 (`CHANNEL_CAP`). If the audio thread is running ahead and the channel is full, `try_send` returns an error and the sim discards that tick's params. The audio thread continues rendering with the last good params — inaudible at 120 Hz sim rate vs. 44 100 Hz audio rate. This prevents any sim-side jitter from causing audio glitches.

---

## Module dependency diagram

```
main.rs
 ├── systems/          (DynamicalSystem trait + all system impls)
 ├── sonification/     (Sonification trait, AudioParams, SonifMode)
 │    └── depends on synth::OscShape (for voice_shapes field)
 ├── audio/            (AudioEngine, LayerSynth — depends on sonification + synth)
 │    └── synth/       (Oscillator, BiquadFilter, FdnReverb, DelayLine, ...)
 ├── config/           (Config, per-system Config structs)
 ├── ui/               (AppState, draw_ui — depends on config, audio types)
 ├── patches/          (PRESETS constant — depends on config)
 ├── presets/          (preset→Config application)
 ├── arrangement/      (scene interpolation — depends on config)
 └── plugin/           (nih-plug wrapper — depends on audio + config)
```

`systems` and `synth` are leaf modules with no intra-crate dependencies. `sonification` sits between them and `audio`. `main.rs` is the only place that instantiates and connects the threads.

---

## Sim→audio channel detail

```rust
// main.rs
const CHANNEL_CAP: usize = 16;
let (tx, rx) = bounded::<[Option<AudioParams>; 3]>(CHANNEL_CAP);
```

Each message is an array of three optional `AudioParams`, one per polyphony layer (`None` = layer inactive). The sim thread calls `tx.try_send(batch)` at the end of every 120 Hz tick. The audio thread calls `rx.try_recv()` at the start of each cpal buffer callback and stores the result in a `latest: [Option<AudioParams>; 3]` local variable.

If `try_recv` returns `Empty`, the audio thread reuses `latest` — the synth state (oscillator phases, filter state, etc.) continues evolving smoothly from where it left off, so the output is perceptually continuous even without a new sim frame. If the channel is full (sim is producing faster than the audio thread is consuming, which should not happen in practice), `try_send` silently drops the frame. This ensures the sim thread never blocks waiting for audio.
