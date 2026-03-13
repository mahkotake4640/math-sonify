mod systems;
mod sonification;
mod synth;
mod audio;
mod config;
mod ui;
mod presets;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::bounded;
use parking_lot::Mutex;

use crate::config::{Config, load_config};
use crate::systems::*;
use crate::sonification::{
    AudioParams, Sonification,
    DirectMapping, OrbitalResonance, GranularMapping, SpectralMapping,
    chord_intervals_for,
};
use crate::audio::AudioEngine;
use crate::ui::{AppState, SharedState, draw_ui};

// Channel capacity (sim -> audio). Only the latest value matters.
const CHANNEL_CAP: usize = 16;
// How many trajectory points to store for visualization.
const VIZ_HISTORY: usize = 1024;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config_path = std::path::PathBuf::from("config.toml");
    let config = load_config(&config_path);

    // Shared state for UI <-> sim communication
    let shared = Arc::new(Mutex::new(AppState::new(config.clone())));

    // Visualization history (shared between sim and UI)
    let viz_history: Arc<Mutex<Vec<(f32, f32, f32)>>> =
        Arc::new(Mutex::new(Vec::with_capacity(VIZ_HISTORY)));

    // Waveform capture buffer
    let waveform_buf: Arc<parking_lot::Mutex<Vec<f32>>> =
        Arc::new(parking_lot::Mutex::new(Vec::with_capacity(2048)));

    // Channel: sim thread -> audio thread
    let (tx, rx) = bounded::<AudioParams>(CHANNEL_CAP);

    // Audio engine
    let _audio = AudioEngine::start(
        rx,
        config.audio.sample_rate,
        config.audio.reverb_wet,
        config.audio.delay_ms,
        config.audio.delay_feedback,
        config.audio.master_volume,
        waveform_buf.clone(),
    )?;

    // Simulation thread
    let shared_sim = shared.clone();
    let viz_sim = viz_history.clone();
    thread::spawn(move || {
        sim_thread(shared_sim, viz_sim, tx);
    });

    // UI (eframe -- runs on main thread)
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_title("Math Sonify"),
        ..Default::default()
    };

    eframe::run_native(
        "Math Sonify",
        options,
        Box::new(move |_cc| {
            Box::new(SonifyApp {
                shared: shared.clone(),
                viz_history: viz_history.clone(),
                waveform_buf: waveform_buf.clone(),
            })
        }),
    ).map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

struct SonifyApp {
    shared: SharedState,
    viz_history: Arc<Mutex<Vec<(f32, f32, f32)>>>,
    waveform_buf: Arc<parking_lot::Mutex<Vec<f32>>>,
}

impl eframe::App for SonifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let points = self.viz_history.lock().clone();
        draw_ui(ctx, &self.shared, &points, &self.waveform_buf);
        ctx.request_repaint_after(Duration::from_millis(33)); // ~30 fps UI
    }
}

// ---------------------------------------------------------------------------
// Simulation thread
// ---------------------------------------------------------------------------

fn sim_thread(
    shared: SharedState,
    viz: Arc<Mutex<Vec<(f32, f32, f32)>>>,
    tx: crossbeam_channel::Sender<AudioParams>,
) {
    let initial_config = shared.lock().config.clone();
    let mut system = build_system(&initial_config);
    let mut mapper = build_mapper(&initial_config.sonification.mode);

    let control_rate_hz = 120.0f64;
    let control_period = Duration::from_secs_f64(1.0 / control_rate_hz);
    let mut next_tick = Instant::now();

    loop {
        let now = Instant::now();
        if now < next_tick {
            thread::sleep(next_tick - now);
        }
        next_tick += control_period;

        let (paused, config, sys_changed, mode_changed) = {
            let mut st = shared.lock();
            let p = st.paused;
            let c = st.config.clone();
            let sc = st.system_changed;
            let mc = st.mode_changed;
            st.system_changed = false;
            st.mode_changed = false;
            (p, c, sc, mc)
        };

        if paused { continue; }
        if sys_changed  { system = build_system(&config); }
        if mode_changed { mapper = build_mapper(&config.sonification.mode); }

        // Integrate enough steps to cover one control period at the configured speed
        let steps = ((config.system.speed / control_rate_hz) / config.system.dt)
            .round() as usize;
        for _ in 0..steps.clamp(1, 10_000) {
            system.step(config.system.dt);
        }

        // Update visualization
        {
            let state = system.state();
            if state.len() >= 2 {
                let mut vh = viz.lock();
                if vh.len() >= VIZ_HISTORY { vh.drain(0..256); }
                let speed_norm = (system.speed() as f32 / 100.0).clamp(0.0, 1.0);
                vh.push((state[0] as f32, state[1] as f32, speed_norm));
            }
        }

        // Map state to audio params and send (non-blocking, drop if full)
        let mut params = mapper.map(system.state(), system.speed(), &config.sonification);

        // Fill harmony fields from config
        params.transpose_semitones = config.sonification.transpose_semitones;
        params.chord_intervals = chord_intervals_for(&config.sonification.chord_mode);
        params.voice_levels = config.sonification.voice_levels;
        params.portamento_ms = config.sonification.portamento_ms;

        // Fill audio effect fields from config (so audio thread reads them dynamically)
        params.master_volume = config.audio.master_volume;
        params.reverb_wet = config.audio.reverb_wet;
        params.delay_ms = config.audio.delay_ms;
        params.delay_feedback = config.audio.delay_feedback;

        // Update shared state from simulation
        {
            let mut st = shared.lock();
            st.chaos_level = params.chaos_level;
            st.current_state = system.state().to_vec();
            st.current_deriv = system.current_deriv();
            // For Kuramoto: store phases and order param
            if config.system.name == "kuramoto" {
                st.kuramoto_phases = system.state().to_vec();
                // order_param exposed via current_deriv indirectly; need cast
                // We detect kuramoto by name and store order_param separately
                st.order_param = {
                    // Compute from phases stored in current_state
                    let phases = &st.current_state;
                    let n = phases.len() as f64;
                    if n > 0.0 {
                        let sin_sum: f64 = phases.iter().map(|&th| th.sin()).sum();
                        let cos_sum: f64 = phases.iter().map(|&th| th.cos()).sum();
                        (sin_sum.powi(2) + cos_sum.powi(2)).sqrt() / n
                    } else {
                        0.0
                    }
                };
            }
        }

        let _ = tx.try_send(params);
    }
}

fn build_system(config: &Config) -> Box<dyn DynamicalSystem> {
    match config.system.name.as_str() {
        "rossler"         => Box::new(Rossler::new(config.rossler.a, config.rossler.b, config.rossler.c)),
        "double_pendulum" => Box::new(DoublePendulum::new(
            config.double_pendulum.m1, config.double_pendulum.m2,
            config.double_pendulum.l1, config.double_pendulum.l2,
        )),
        "geodesic_torus"  => Box::new(GeodesicTorus::new(config.geodesic_torus.big_r, config.geodesic_torus.r)),
        "kuramoto"        => Box::new(Kuramoto::new(config.kuramoto.n_oscillators, config.kuramoto.coupling)),
        "three_body"      => Box::new(ThreeBody::new([1.0, 1.0, 1.0])),
        _                 => Box::new(Lorenz::new(config.lorenz.sigma, config.lorenz.rho, config.lorenz.beta)),
    }
}

fn build_mapper(mode: &str) -> Box<dyn Sonification> {
    match mode {
        "orbital"  => Box::new(OrbitalResonance::new()),
        "granular" => Box::new(GranularMapping::new()),
        "spectral" => Box::new(SpectralMapping::new()),
        _          => Box::new(DirectMapping::new()),
    }
}
