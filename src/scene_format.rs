//! `.msonify` scene file format — serializable snapshot of a complete performance.
//!
//! A `.msonify` file is a TOML document that captures everything needed to
//! restore a performance session: system parameters, synthesis settings,
//! arrangement data, and a log of recorded events.
//!
//! # Example `.msonify` file
//!
//! ```toml
//! [meta]
//! version = "1.0"
//! created_at = "2026-03-22T12:00:00Z"
//! name = "Lorenz Afternoon"
//!
//! [system]
//! name = "lorenz"
//! dt = 0.001
//! speed = 1.0
//! sigma = 10.0
//! rho = 28.0
//! beta = 2.667
//!
//! [synth]
//! mode = "fm"
//! scale = "pentatonic"
//! base_frequency = 110.0
//! octave_range = 2.5
//! master_volume = 0.75
//! reverb_wet = 0.55
//!
//! [[events]]
//! time_s = 0.0
//! kind = "param_change"
//! param = "rho"
//! value = 28.0
//!
//! [[events]]
//! time_s = 12.5
//! kind = "param_change"
//! param = "rho"
//! value = 35.0
//! ```

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ── Scene metadata ────────────────────────────────────────────────────────────

/// Top-level `.msonify` scene document.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MsonifyScene {
    /// Metadata about the scene.
    pub meta: SceneMeta,
    /// Dynamical system configuration.
    pub system: SystemSnapshot,
    /// Synthesis / audio configuration.
    pub synth: SynthSnapshot,
    /// Arrangement: time-ordered list of events.
    #[serde(default)]
    pub events: Vec<SceneEvent>,
    /// Arbitrary extra key-value pairs (for forward compatibility).
    #[serde(default, flatten)]
    pub extra: HashMap<String, toml::Value>,
}

/// Metadata section of a `.msonify` scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneMeta {
    /// Format version string.
    #[serde(default = "default_version")]
    pub version: String,
    /// Human-readable name for this scene.
    #[serde(default)]
    pub name: String,
    /// ISO-8601 creation timestamp.
    #[serde(default)]
    pub created_at: String,
    /// Optional author name.
    #[serde(default)]
    pub author: String,
    /// Free-form notes.
    #[serde(default)]
    pub notes: String,
    /// Duration of the recorded performance in seconds (0 = not recorded).
    #[serde(default)]
    pub duration_s: f64,
}

impl Default for SceneMeta {
    fn default() -> Self {
        Self {
            version: default_version(),
            name: "Untitled Scene".into(),
            created_at: String::new(),
            author: String::new(),
            notes: String::new(),
            duration_s: 0.0,
        }
    }
}

fn default_version() -> String {
    "1.0".into()
}

// ── System snapshot ───────────────────────────────────────────────────────────

/// Snapshot of dynamical system parameters.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemSnapshot {
    /// System identifier (e.g. `"lorenz"`, `"rossler"`, `"double_pendulum"`).
    #[serde(default)]
    pub name: String,
    /// Integration timestep.
    #[serde(default = "default_dt")]
    pub dt: f64,
    /// Playback speed multiplier.
    #[serde(default = "default_one")]
    pub speed: f64,
    /// Initial state vector `[x0, y0, z0, ...]`.
    #[serde(default)]
    pub initial_state: Vec<f64>,
    /// Named parameters (system-specific, e.g. `sigma`, `rho`, `beta`).
    #[serde(default, flatten)]
    pub params: HashMap<String, f64>,
}

fn default_dt() -> f64 { 0.001 }
fn default_one() -> f64 { 1.0 }

// ── Synth snapshot ────────────────────────────────────────────────────────────

/// Snapshot of synthesis and audio parameters.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SynthSnapshot {
    /// Sonification mode (e.g. `"fm"`, `"granular"`, `"direct"`).
    #[serde(default)]
    pub mode: String,
    /// Musical scale name.
    #[serde(default)]
    pub scale: String,
    /// Base frequency in Hz.
    #[serde(default = "default_base_freq")]
    pub base_frequency: f64,
    /// Octave range for pitch mapping.
    #[serde(default = "default_one")]
    pub octave_range: f64,
    /// Master volume (0–1).
    #[serde(default = "default_volume")]
    pub master_volume: f64,
    /// Reverb wet amount (0–1).
    #[serde(default)]
    pub reverb_wet: f64,
    /// Delay time in milliseconds.
    #[serde(default)]
    pub delay_ms: f64,
    /// Delay feedback (0–1).
    #[serde(default)]
    pub delay_feedback: f64,
    /// Physical synthesis mode if active (`"plucked"`, `"tube"`, or `""`).
    #[serde(default)]
    pub physical_mode: String,
    /// Composition engine enabled.
    #[serde(default)]
    pub composition_enabled: bool,
    /// Collab server address (empty = disabled).
    #[serde(default)]
    pub collab_addr: String,
}

fn default_base_freq() -> f64 { 110.0 }
fn default_volume() -> f64 { 0.75 }

// ── Events ────────────────────────────────────────────────────────────────────

/// A recorded performance event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEvent {
    /// Timestamp in seconds from session start.
    pub time_s: f64,
    /// Event type identifier.
    pub kind: String,
    /// Optional parameter name (for `param_change` events).
    #[serde(default)]
    pub param: String,
    /// Optional numeric value.
    #[serde(default)]
    pub value: f64,
    /// Optional free-form label.
    #[serde(default)]
    pub label: String,
}

// ── Known event kinds ─────────────────────────────────────────────────────────

/// Well-known event `kind` strings.
pub mod event_kind {
    pub const PARAM_CHANGE: &str = "param_change";
    pub const PRESET_LOAD: &str = "preset_load";
    pub const SYSTEM_CHANGE: &str = "system_change";
    pub const KEY_CHANGE: &str = "key_change";
    pub const NOTE_ON: &str = "note_on";
    pub const NOTE_OFF: &str = "note_off";
    pub const TEMPO_CHANGE: &str = "tempo_change";
    pub const MARKER: &str = "marker";
}

// ── Import / export ───────────────────────────────────────────────────────────

/// Error type for scene serialization / deserialization.
#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),
}

impl MsonifyScene {
    /// Serialize the scene to a TOML string.
    pub fn to_toml(&self) -> Result<String, SceneError> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Deserialize a scene from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self, SceneError> {
        Ok(toml::from_str(s)?)
    }

    /// Save the scene to a `.msonify` file at `path`.
    pub fn save(&self, path: &Path) -> Result<(), SceneError> {
        let s = self.to_toml()?;
        std::fs::write(path, s)?;
        log::info!("[scene] saved to {}", path.display());
        Ok(())
    }

    /// Load a scene from a `.msonify` file at `path`.
    pub fn load(path: &Path) -> Result<Self, SceneError> {
        let s = std::fs::read_to_string(path)?;
        let scene = Self::from_toml(&s)?;
        log::info!(
            "[scene] loaded '{}' from {} ({} events)",
            scene.meta.name,
            path.display(),
            scene.events.len()
        );
        Ok(scene)
    }

    /// Append a recorded event to the event list.
    pub fn record_event(&mut self, event: SceneEvent) {
        self.events.push(event);
    }

    /// Record a parameter change.
    pub fn record_param(&mut self, time_s: f64, param: &str, value: f64) {
        self.record_event(SceneEvent {
            time_s,
            kind: event_kind::PARAM_CHANGE.into(),
            param: param.into(),
            value,
            label: String::new(),
        });
    }

    /// Record a named marker (e.g. "intro", "drop").
    pub fn record_marker(&mut self, time_s: f64, label: &str) {
        self.record_event(SceneEvent {
            time_s,
            kind: event_kind::MARKER.into(),
            param: String::new(),
            value: 0.0,
            label: label.into(),
        });
    }

    /// Return all events of a given kind in time order.
    pub fn events_of_kind(&self, kind: &str) -> Vec<&SceneEvent> {
        self.events.iter().filter(|e| e.kind == kind).collect()
    }

    /// Return the total duration of the recorded performance (last event time).
    pub fn recorded_duration(&self) -> f64 {
        self.events
            .iter()
            .map(|e| e.time_s)
            .fold(0.0_f64, f64::max)
    }
}

// ── Builder ───────────────────────────────────────────────────────────────────

/// Fluent builder for [`MsonifyScene`].
pub struct SceneBuilder {
    scene: MsonifyScene,
}

impl SceneBuilder {
    pub fn new() -> Self {
        Self {
            scene: MsonifyScene::default(),
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.scene.meta.name = name.into();
        self
    }

    pub fn author(mut self, author: &str) -> Self {
        self.scene.meta.author = author.into();
        self
    }

    pub fn system(mut self, system: SystemSnapshot) -> Self {
        self.scene.system = system;
        self
    }

    pub fn synth(mut self, synth: SynthSnapshot) -> Self {
        self.scene.synth = synth;
        self
    }

    pub fn build(self) -> MsonifyScene {
        self.scene
    }
}

impl Default for SceneBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_scene() -> MsonifyScene {
        let mut scene = MsonifyScene::default();
        scene.meta.name = "Test Scene".into();
        scene.system.name = "lorenz".into();
        scene.system.params.insert("rho".into(), 28.0);
        scene.synth.mode = "fm".into();
        scene.synth.master_volume = 0.75;
        scene.record_param(0.0, "rho", 28.0);
        scene.record_param(10.0, "rho", 35.0);
        scene.record_marker(5.0, "drop");
        scene
    }

    #[test]
    fn test_roundtrip_toml() {
        let scene = make_scene();
        let toml = scene.to_toml().unwrap();
        let restored = MsonifyScene::from_toml(&toml).unwrap();
        assert_eq!(restored.meta.name, "Test Scene");
        assert_eq!(restored.system.name, "lorenz");
        assert!((restored.system.params["rho"] - 28.0).abs() < 1e-10);
        assert_eq!(restored.events.len(), 3);
    }

    #[test]
    fn test_events_of_kind() {
        let scene = make_scene();
        let params = scene.events_of_kind(event_kind::PARAM_CHANGE);
        assert_eq!(params.len(), 2);
        let markers = scene.events_of_kind(event_kind::MARKER);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].label, "drop");
    }

    #[test]
    fn test_recorded_duration() {
        let scene = make_scene();
        let dur = scene.recorded_duration();
        assert!((dur - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let scene = make_scene();
        let dir = std::env::temp_dir();
        let path = dir.join("test_scene.msonify");
        scene.save(&path).unwrap();
        let loaded = MsonifyScene::load(&path).unwrap();
        assert_eq!(loaded.meta.name, "Test Scene");
        assert_eq!(loaded.events.len(), 3);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_builder() {
        let mut sys = SystemSnapshot::default();
        sys.name = "rossler".into();
        let scene = SceneBuilder::new()
            .name("Builder Test")
            .author("Test Author")
            .system(sys)
            .build();
        assert_eq!(scene.meta.name, "Builder Test");
        assert_eq!(scene.system.name, "rossler");
    }
}
