# math-sonify

[![CI](https://github.com/Mattbusel/math-sonify/actions/workflows/ci.yml/badge.svg)](https://github.com/Mattbusel/math-sonify/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

Real-time sonification of chaotic dynamical systems. The differential equations run continuously; the audio is a direct mapping of their evolving state. Not a preset synthesizer with math-themed names -- the Lorenz attractor is actually integrating, the Kuramoto coupling constant is live, the Three-Body gravitational problem is being stepped forward at 120 Hz.

---

## Architecture

```
ODE Solver (120 Hz, sim thread)
    |
    |  Lorenz / Rossler / Duffing / Kuramoto / Three-Body / ...
    |  RK4 or leapfrog integration per configured dt
    |
    v
Parameter Morphing (arrangement layer)
    |
    |  Scene arranger linearly interpolates all numeric config fields
    |  between named snapshots; string fields switch at midpoint
    |
    v
Audio Synthesis (audio thread, 44100 / 48000 Hz)
    |
    |  Sonification mode selected per layer:
    |    Direct   -- state quantized to musical scale -> oscillator freqs
    |    Orbital  -- angular velocity + Lyapunov exponent drive pitch
    |    Granular -- trajectory speed -> grain density and pitch
    |    Spectral -- state -> 32-partial additive envelope
    |    FM       -- attractor drives carrier/modulator ratio and index
    |    Vocal    -- state interpolates between vowel formant positions
    |    Waveguide-- Karplus-Strong string with chaotic modulation
    |
    |  Per-layer DSP: Waveshaper -> Bitcrusher
    |  Master bus:    LP filter -> Delay -> Chorus -> FDN Reverb -> Limiter
    |
    v
DAW (VST3 / CLAP plugin) or Desktop (standalone cpal output)
```

Communication between the simulation thread and audio thread uses a bounded crossbeam channel. The audio callback never blocks; it calls `try_recv` and renders silence on a miss.

---

## Supported mathematical systems

| System | Equations (abbreviated) | Notes |
|--------|--------------------------|-------|
| Lorenz | dx/dt = sigma*(y-x), dy/dt = x*(rho-z)-y, dz/dt = xy-beta*z | Classic butterfly attractor; chaos onset near rho=24.74 |
| Rossler | dx/dt = -y-z, dy/dt = x+a*y, dz/dt = b+z*(x-c) | Spiral attractor; period-doubling route to chaos as c increases |
| Double Pendulum | Lagrangian mechanics, 4D state (theta1, theta2, omega1, omega2) | Gravitational chaos; quasi-periodic pockets exist |
| Geodesic Torus | Geodesic flow on a flat torus with irrational winding number | Ergodic; trajectory never closes |
| Kuramoto | d(theta_i)/dt = omega_i + (K/N)*sum_j sin(theta_j - theta_i) | N coupled oscillators; synchronization transition at critical K |
| Three-Body | Newtonian gravity for 3 point masses in 2D | Leapfrog integrator; figure-8 initial conditions |
| Duffing | dx/dt = v, dv/dt = -delta*v - alpha*x - beta*x^3 + gamma*cos(phi) | Driven nonlinear oscillator; period-doubling cascade |
| Van der Pol | dx/dt = v, dv/dt = mu*(1-x^2)*v - x | Limit-cycle oscillator; relaxation oscillations at high mu |
| Halvorsen | Cyclic 3D attractor with cyclic symmetry | Dense spiral attractor |
| Aizawa | Six-parameter torus-like attractor | Slow toroidal wobble |
| Chua | Piecewise-linear electronic circuit model | Double-scroll chaotic attractor |
| Hindmarsh-Rose | Three-variable neuron firing model | Bursting and spiking regimes |
| Lorenz-96 | N-variable weather prediction toy model | Spatiotemporal chaos at F > 8 |
| Mackey-Glass | dx/dt = beta*x(t-tau)/(1+x^n) - gamma*x | History-dependent delay DDE |
| Nose-Hoover | Thermostatted Hamiltonian system | Conservative chaos |
| Coupled Map Lattice | Logistic map on a 1D lattice with nearest-neighbor coupling | Spatiotemporal chaos |
| Henon Map | Discrete map: x -> 1 - a*x^2 + y, y -> b*x | Fractal strange attractor |
| Custom ODE | User-defined 3-variable system via text input | Expression parser: sin/cos/exp/pow/pi/e |
| Fractional Lorenz | Lorenz with derivative order alpha in (0.5, 1.0] | History-dependent; active research area |

---

## Installation

### From crates.io (recommended)

Requires [Rust](https://rustup.rs/) 1.75 or later.

```bash
cargo install math-sonify
math-sonify
```

Audio starts immediately using the system default output device.

### Pre-built binaries

Download a pre-built executable from the [latest GitHub release](https://github.com/Mattbusel/math-sonify/releases/latest).

---

## Quickstart

### Run the standalone application

```bash
git clone https://github.com/Mattbusel/math-sonify
cd math-sonify
cargo run --release
```

Audio starts immediately using the system default output device at 44100 Hz. The Lorenz attractor runs by default. Use the GUI to switch systems, adjust parameters, and configure effects.

### Headless mode

```bash
cargo run --release -- --headless --duration 60 --output clip.wav
```

Renders 60 seconds of audio to a WAV file with no GUI.

### Build the VST3 / CLAP plugin

```bash
cargo build --release --lib
```

The plugin shared library is written to `target/release/`. Copy it to your DAW plugin folder and rescan.

- Windows: `math_sonify_plugin.dll` to `C:\Program Files\Common Files\VST3\`
- Linux: `libmath_sonify_plugin.so` to `~/.vst3/`
- macOS: `libmath_sonify_plugin.dylib` to `~/Library/Audio/Plug-Ins/VST3/`

---

## Configuration reference

The application reads `config.toml` from the working directory on startup. All fields are optional; missing values use their defaults. The file is watched for changes; edits take effect without restarting.

```toml
[system]
name  = "lorenz"  # active system at startup
dt    = 0.001     # ODE integration time step (clamped 0.0001..0.1)
speed = 1.0       # simulation speed multiplier (0..100)

[lorenz]
sigma = 10.0
rho   = 28.0
beta  = 2.6667

[rossler]
a = 0.2
b = 0.2
c = 5.7

[kuramoto]
n_oscillators = 8
coupling      = 1.5   # K; synchronization threshold approx 1.0 for this distribution

[duffing]
delta = 0.3
alpha = -1.0
beta  = 1.0
gamma = 0.5
omega = 1.2

[audio]
sample_rate      = 44100   # 44100 or 48000
buffer_size      = 512
reverb_wet       = 0.4     # 0..1
delay_ms         = 300.0   # 1..5000
delay_feedback   = 0.3     # 0..0.99
master_volume    = 0.7     # 0..1
bit_depth        = 16.0    # 1..32 (32 = bypass)
rate_crush       = 0.0     # 0..1
chorus_mix       = 0.0
chorus_rate      = 0.5     # Hz
chorus_depth     = 3.0     # ms
waveshaper_drive = 1.0     # 0..100
waveshaper_mix   = 0.0

[sonification]
mode                = "direct"
scale               = "pentatonic"  # pentatonic | chromatic | just_intonation | microtonal
                                    # | edo19 | edo31 | edo24 | whole_tone | phrygian | lydian
base_frequency      = 220.0         # Hz, root of the scale
octave_range        = 3.0
transpose_semitones = 0.0
chord_mode          = "none"        # none | major | minor | power | sus2 | octave | dom7
portamento_ms       = 80.0
voice_levels        = [1.0, 0.8, 0.6, 0.4]
voice_shapes        = ["sine", "sine", "sine", "sine"]

[viz]
trail_length = 800    # points in the phase portrait trail
projection   = "xy"   # xy | xz | yz | 3d
glow         = true
theme        = "neon" # neon | amber | ice | mono
```

---

## Building and testing

```bash
# Run all unit and integration tests (no display required)
cargo test --lib --tests

# Run only ODE solver accuracy tests
cargo test --lib -- lorenz_stays_on_attractor

# Release build
cargo build --release --lib
```

The test suite covers: ODE solver accuracy (attractor bounds, energy conservation, synchronization thresholds), parameter morphing, audio oscillator amplitude bounds, config parsing, hot-reload, and scene arranger timeline consistency.

---

## GUI layout

The application has three top-level tabs:

- **SYNTH** -- system selector, parameter sliders, sonification mode, scale, effects chain.
- **MIXER** -- per-layer volume/pan/ADSR, master effects, VU meters, WAV export, clip save.
- **ARRANGE** -- scene timeline, morph time controls, AUTO arrangement generator.
- **MATH VIEW** -- live phase portrait, bifurcation diagram, custom ODE text input, state readout.
- **WAVEFORM** -- oscilloscope and spectrum analyzer.

Performance mode (press F) switches to fullscreen phase portrait only.

---

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| `F` | Toggle fullscreen performance mode (phase portrait only) |
| `Space` | Pause / resume simulation |
| `R` | Reset attractor to default initial condition |
| `S` | Save clip (last 60 seconds as WAV + PNG) |
| `Ctrl+S` | Save current configuration to `config.toml` |
| `1` – `7` | Switch sonification mode (Direct, Orbital, Granular, Spectral, FM, Vocal, Waveguide) |
| `←` / `→` | Previous / next dynamical system |
| `↑` / `↓` | Increase / decrease simulation speed by 10% |
| `E` | Toggle Evolve (autonomous parameter wandering) |
| `A` | Toggle AUTO arrangement playback |
| `P` | Play / stop scene arranger |
| `Escape` | Exit fullscreen |

---

## Troubleshooting

### No audio / audio device not found

- **Check your default output device.** math-sonify uses the system default audio output (`cpal::default_host().default_output_device()`). Make sure a device is selected in your OS audio settings.
- **Exclusive mode conflicts (Windows).** If another application has taken the audio device in exclusive mode (e.g., some games or audio interfaces), close that application first.
- **ALSA errors on Linux.** Install `libasound2-dev` (`sudo apt install libasound2-dev`) and ensure your user is in the `audio` group (`sudo usermod -aG audio $USER`, then log out and back in).
- **Sample rate mismatch.** If you see an `AudioDeviceError` in the log, your device may not support 44100 Hz. Set `sample_rate = 48000` in `config.toml`.

### High CPU usage

- Reduce `buffer_size` in `config.toml` (try `1024` or `2048`).
- Disable Evolve if you are not using it.
- For the Three-Body and Lorenz96 systems, reduce `system.speed` to lower the integration rate.

### Distorted / clipping audio

- Lower `audio.master_volume` in `config.toml` (default 0.7).
- The Lookahead Limiter on the master bus prevents true clipping; audible distortion usually means the waveshaper drive is too high — set `waveshaper_drive = 1.0` and `waveshaper_mix = 0.0`.

### The phase portrait is blank

- The attractor needs a few seconds to build up a trail from scratch. After a reset press (`R`) wait 2–3 seconds.
- If the system diverges (all-zero or all-NaN state), the engine resets automatically; this is logged as an `OdeIntegrationError`.

### Config file not loading

- math-sonify looks for `config.toml` in the **current working directory** at startup. Run the binary from the directory containing your `config.toml`, or pass an absolute path via `--config /path/to/config.toml`.
- Validation errors (values out of range) are logged as warnings; the field is clamped to its valid range rather than rejected.

### VST3 / CLAP plugin not appearing in DAW

- Ensure you copied the `.dll` (Windows) or `.so` (Linux) to the correct system VST3 folder and triggered a plugin rescan in your DAW.
- Some DAWs require a full restart after installing new plugins.
- Check that the plugin was built with `cargo build --release --lib` (not `--bin`).

---

## License

MIT. See [LICENSE](LICENSE).

---

Built with [Rust](https://www.rust-lang.org), [cpal](https://github.com/RustAudio/cpal), [egui](https://github.com/emilk/egui), [nih-plug](https://github.com/robbert-vdh/nih-plug), [crossbeam](https://github.com/crossbeam-rs/crossbeam), [parking_lot](https://github.com/Amanieu/parking_lot), [hound](https://github.com/ruuda/hound).
