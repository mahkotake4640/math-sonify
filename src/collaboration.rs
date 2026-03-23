//! Collaborative Performance Mode
//!
//! Multiple performers connect to a shared "session" and influence each
//! other's attractor parameters in real time.  One performer's chaotic
//! system output can modulate another's parameters.
//!
//! ## Architecture
//!
//! ```text
//! Performer A                   Performer B
//! CollaborationClient           CollaborationClient
//!       |                             |
//!       | SessionMessage (JSON)       |
//!       v                             v
//!  [WebSocket / UDP layer]  <--> [WebSocket / UDP layer]
//!       |                             |
//!       +----------> CollaborationSession <-----------+
//!                    (shared session state)
//! ```
//!
//! ## Network protocol
//!
//! Messages are JSON-encoded variants of [`SessionMessage`]:
//!
//! | Message | Direction | Purpose |
//! |---|---|---|
//! | `JoinSession`    | client -> server | announce presence |
//! | `LeaveSession`   | client -> server | clean disconnect |
//! | `StateUpdate`    | client -> all    | push current attractor state |
//! | `ParameterSync`  | any -> any       | nudge named parameters on peers |
//! | `ChatMessage`    | any -> all       | optional text chat |
//! | `KickOff`        | host -> all      | synchronised start event |
//!
//! This module is transport-agnostic: it produces and parses JSON strings.
//! The actual network I/O (WebSocket, UDP, OSC) is handled externally so
//! that the core DSP/sim code stays real-time-safe and dependency-free.

use std::collections::HashMap;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

// ---- Performer state --------------------------------------------------------

/// The live state of one performer in a session.
///
/// Sent as part of every [`SessionMessage::StateUpdate`].  The fields are
/// intentionally flat and `f64`/`f32` so they can be interpolated by peers
/// to smooth latency jitter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformerState {
    /// Stable unique identifier (UUID or user-chosen short string).
    pub performer_id: String,
    /// Human-readable display name shown in the session UI.
    pub display_name: String,
    /// Name of the dynamical system this performer is running.
    pub system_name: String,
    /// Current x-coordinate of the attractor.
    pub x: f64,
    /// Current y-coordinate of the attractor.
    pub y: f64,
    /// Current z-coordinate of the attractor.
    pub z: f64,
    /// Current playback tempo in beats per minute.
    pub tempo_bpm: f64,
    /// Master volume level (0.0 - 1.0).
    pub volume: f32,
    /// RGB colour used to visually differentiate this performer.
    pub color: [u8; 3],
}

impl PerformerState {
    /// Create a performer state with sensible defaults.
    pub fn new(performer_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            performer_id: performer_id.into(),
            display_name: display_name.into(),
            system_name: "lorenz".to_string(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
            tempo_bpm: 120.0,
            volume: 0.75,
            color: [128, 200, 255],
        }
    }

    /// Update attractor coordinates.
    pub fn set_xyz(&mut self, x: f64, y: f64, z: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
    }
}

// ---- Session messages -------------------------------------------------------

/// All message variants exchanged between performers.
///
/// Serialised with the `"type"` tag so the JSON discriminant is the variant
/// name, e.g. `{"type":"JoinSession","session_id":"abc","performer":{...}}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum SessionMessage {
    /// Performer requests to join the named session.
    JoinSession {
        session_id: String,
        performer: PerformerState,
    },
    /// Performer announces a clean disconnect.
    LeaveSession {
        performer_id: String,
    },
    /// Periodic push of attractor state (high-frequency, lossy OK).
    StateUpdate {
        performer: PerformerState,
    },
    /// Push a set of named parameters to all peers (or a specific target).
    ParameterSync {
        params: HashMap<String, f64>,
    },
    /// Optional text chat within the session.
    ChatMessage {
        performer_id: String,
        text: String,
    },
    /// Host signals a synchronised start: all performers should begin at
    /// the given BPM on receipt.
    KickOff {
        session_id: String,
        bpm: f64,
    },
}

// ---- Server-side session ----------------------------------------------------

/// Server-side model of one shared performance session.
///
/// Tracks which performers are present and their last-known state.
/// Thread-safety (locking) is left to the caller; wrap in `Arc<Mutex<_>>` for
/// multi-threaded use.
pub struct CollaborationSession {
    /// Unique identifier for this session (e.g. a short random slug).
    pub session_id: String,
    /// Map from `performer_id` to last-known [`PerformerState`].
    pub performers: HashMap<String, PerformerState>,
    /// Timestamp at which the session was created.
    pub created_at: SystemTime,
    /// Maximum number of performers allowed (default: 8).
    pub max_performers: usize,
}

impl CollaborationSession {
    /// Create a new empty session with the given identifier.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            performers: HashMap::new(),
            created_at: SystemTime::now(),
            max_performers: 8,
        }
    }

    /// Add a performer to the session.
    ///
    /// Returns `Err` if the session is full or if the performer ID is already
    /// present (use [`update_state`] to refresh existing performers).
    pub fn join(&mut self, performer: PerformerState) -> Result<(), String> {
        if self.is_full() {
            return Err(format!(
                "Session '{}' is full ({} performers)",
                self.session_id, self.max_performers
            ));
        }
        if self.performers.contains_key(&performer.performer_id) {
            return Err(format!(
                "Performer '{}' is already in session '{}'",
                performer.performer_id, self.session_id
            ));
        }
        self.performers.insert(performer.performer_id.clone(), performer);
        Ok(())
    }

    /// Remove a performer from the session.  Silently ignores unknown IDs.
    pub fn leave(&mut self, performer_id: &str) {
        self.performers.remove(performer_id);
    }

    /// Update the state for an existing performer.
    ///
    /// If the performer is not yet in the session they are inserted (allowing
    /// implicit join on first state push).
    pub fn update_state(&mut self, performer: PerformerState) {
        self.performers.insert(performer.performer_id.clone(), performer);
    }

    /// Build a [`SessionMessage::ParameterSync`] containing a snapshot of
    /// the mean attractor coordinates across all performers.
    ///
    /// Useful as a "broadcast" message that lets new joiners quickly orient
    /// to the current collective state.
    pub fn broadcast_message(&self) -> SessionMessage {
        let n = self.performers.len().max(1) as f64;
        let mut params = HashMap::new();
        let (sx, sy, sz) = self.performers.values().fold((0.0, 0.0, 0.0), |acc, p| {
            (acc.0 + p.x, acc.1 + p.y, acc.2 + p.z)
        });
        params.insert("mean_x".to_string(), sx / n);
        params.insert("mean_y".to_string(), sy / n);
        params.insert("mean_z".to_string(), sz / n);
        params.insert("performer_count".to_string(), self.performers.len() as f64);
        SessionMessage::ParameterSync { params }
    }

    /// Return `true` if the session has reached `max_performers`.
    pub fn is_full(&self) -> bool {
        self.performers.len() >= self.max_performers
    }

    /// Return the number of performers currently in the session.
    pub fn performer_count(&self) -> usize {
        self.performers.len()
    }

    /// Return the state for a specific performer, if present.
    pub fn performer(&self, id: &str) -> Option<&PerformerState> {
        self.performers.get(id)
    }

    /// Return all performer states as a sorted `Vec` (sorted by `performer_id`
    /// for deterministic ordering).
    pub fn all_performers(&self) -> Vec<&PerformerState> {
        let mut v: Vec<&PerformerState> = self.performers.values().collect();
        v.sort_by(|a, b| a.performer_id.cmp(&b.performer_id));
        v
    }
}

// ---- Client-side connector --------------------------------------------------

/// Client-side representation of the local performer.
///
/// Produces [`SessionMessage`] values ready to serialise and send over any
/// transport.  The actual WebSocket / UDP socket is handled externally so
/// this type contains no async or I/O code and is safe to construct on the
/// audio thread (though you should only update `local_performer` from the
/// simulation thread).
pub struct CollaborationClient {
    /// State of the performer running on this machine.
    pub local_performer: PerformerState,
}

impl CollaborationClient {
    /// Create a new client for the given performer.
    pub fn new(performer: PerformerState) -> Self {
        Self { local_performer: performer }
    }

    /// Build a [`SessionMessage::JoinSession`] for the given session.
    pub fn join_message(&self, session_id: &str) -> SessionMessage {
        SessionMessage::JoinSession {
            session_id: session_id.to_string(),
            performer: self.local_performer.clone(),
        }
    }

    /// Build a [`SessionMessage::LeaveSession`] for the local performer.
    pub fn leave_message(&self) -> SessionMessage {
        SessionMessage::LeaveSession {
            performer_id: self.local_performer.performer_id.clone(),
        }
    }

    /// Build a [`SessionMessage::StateUpdate`] from the current local state.
    pub fn state_update_message(&self) -> SessionMessage {
        SessionMessage::StateUpdate { performer: self.local_performer.clone() }
    }

    /// Build a [`SessionMessage::ChatMessage`] from the local performer.
    pub fn chat_message(&self, text: impl Into<String>) -> SessionMessage {
        SessionMessage::ChatMessage {
            performer_id: self.local_performer.performer_id.clone(),
            text: text.into(),
        }
    }

    /// Serialise a [`SessionMessage`] to a JSON string.
    ///
    /// Returns the raw JSON bytes as a `String`.  The output is compact
    /// (no pretty-printing) to minimise transmission size.
    pub fn serialize_message(msg: &SessionMessage) -> String {
        serde_json::to_string(msg).unwrap_or_else(|e| {
            format!(r#"{{"type":"Error","message":"{e}"}}"#)
        })
    }

    /// Deserialise a JSON string back to a [`SessionMessage`].
    pub fn deserialize_message(json: &str) -> Result<SessionMessage, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Convenience: update xyz and return the resulting state-update message.
    pub fn push_xyz(&mut self, x: f64, y: f64, z: f64) -> SessionMessage {
        self.local_performer.set_xyz(x, y, z);
        self.state_update_message()
    }
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn performer(id: &str) -> PerformerState {
        PerformerState::new(id, id)
    }

    // ---- PerformerState ----------------------------------------------------

    #[test]
    fn performer_state_new_defaults() {
        let p = PerformerState::new("p1", "Alice");
        assert_eq!(p.performer_id, "p1");
        assert_eq!(p.display_name, "Alice");
        assert_eq!(p.system_name,  "lorenz");
        assert_eq!(p.tempo_bpm,    120.0);
    }

    #[test]
    fn performer_state_set_xyz() {
        let mut p = performer("p1");
        p.set_xyz(1.0, 2.0, 3.0);
        assert_eq!((p.x, p.y, p.z), (1.0, 2.0, 3.0));
    }

    // ---- CollaborationSession ----------------------------------------------

    #[test]
    fn session_join_and_count() {
        let mut s = CollaborationSession::new("sess1");
        assert_eq!(s.performer_count(), 0);
        s.join(performer("p1")).unwrap();
        assert_eq!(s.performer_count(), 1);
    }

    #[test]
    fn session_join_duplicate_errors() {
        let mut s = CollaborationSession::new("sess1");
        s.join(performer("p1")).unwrap();
        assert!(s.join(performer("p1")).is_err());
    }

    #[test]
    fn session_full_rejects_join() {
        let mut s = CollaborationSession::new("full");
        s.max_performers = 2;
        s.join(performer("p1")).unwrap();
        s.join(performer("p2")).unwrap();
        assert!(s.is_full());
        assert!(s.join(performer("p3")).is_err());
    }

    #[test]
    fn session_leave_removes_performer() {
        let mut s = CollaborationSession::new("sess1");
        s.join(performer("p1")).unwrap();
        s.leave("p1");
        assert_eq!(s.performer_count(), 0);
    }

    #[test]
    fn session_leave_unknown_no_panic() {
        let mut s = CollaborationSession::new("sess1");
        s.leave("nobody"); // must not panic
    }

    #[test]
    fn session_update_state_inserts_if_absent() {
        let mut s = CollaborationSession::new("sess1");
        let mut p = performer("p1");
        p.x = 5.0;
        s.update_state(p);
        assert_eq!(s.performer("p1").unwrap().x, 5.0);
    }

    #[test]
    fn session_update_state_overwrites() {
        let mut s = CollaborationSession::new("sess1");
        s.join(performer("p1")).unwrap();
        let mut p2 = performer("p1");
        p2.tempo_bpm = 150.0;
        s.update_state(p2);
        assert_eq!(s.performer("p1").unwrap().tempo_bpm, 150.0);
    }

    #[test]
    fn session_broadcast_message_is_parameter_sync() {
        let mut s = CollaborationSession::new("sess1");
        s.join(performer("p1")).unwrap();
        assert!(matches!(s.broadcast_message(), SessionMessage::ParameterSync { .. }));
    }

    #[test]
    fn session_broadcast_includes_mean_coords() {
        let mut s = CollaborationSession::new("sess1");
        let mut p1 = performer("p1");
        p1.x = 2.0;
        let mut p2 = performer("p2");
        p2.x = 4.0;
        s.update_state(p1);
        s.update_state(p2);
        if let SessionMessage::ParameterSync { params } = s.broadcast_message() {
            let mean_x = params["mean_x"];
            assert!((mean_x - 3.0).abs() < 1e-9, "mean_x should be 3.0, got {mean_x}");
        } else {
            panic!("expected ParameterSync");
        }
    }

    #[test]
    fn session_all_performers_sorted() {
        let mut s = CollaborationSession::new("sess1");
        s.join(performer("charlie")).unwrap();
        s.join(performer("alice")).unwrap();
        s.join(performer("bob")).unwrap();
        let ids: Vec<&str> = s.all_performers().iter().map(|p| p.performer_id.as_str()).collect();
        assert_eq!(ids, vec!["alice", "bob", "charlie"]);
    }

    // ---- CollaborationClient -----------------------------------------------

    #[test]
    fn client_join_message_contains_session_id() {
        let c = CollaborationClient::new(performer("p1"));
        if let SessionMessage::JoinSession { session_id, .. } = c.join_message("room42") {
            assert_eq!(session_id, "room42");
        } else {
            panic!("expected JoinSession");
        }
    }

    #[test]
    fn client_leave_message_contains_performer_id() {
        let c = CollaborationClient::new(performer("p1"));
        if let SessionMessage::LeaveSession { performer_id } = c.leave_message() {
            assert_eq!(performer_id, "p1");
        } else {
            panic!("expected LeaveSession");
        }
    }

    #[test]
    fn client_state_update_message() {
        let c = CollaborationClient::new(performer("p1"));
        assert!(matches!(c.state_update_message(), SessionMessage::StateUpdate { .. }));
    }

    #[test]
    fn client_chat_message() {
        let c = CollaborationClient::new(performer("p1"));
        if let SessionMessage::ChatMessage { text, .. } = c.chat_message("hello") {
            assert_eq!(text, "hello");
        } else {
            panic!("expected ChatMessage");
        }
    }

    #[test]
    fn client_push_xyz_updates_state() {
        let mut c = CollaborationClient::new(performer("p1"));
        let msg = c.push_xyz(1.5, 2.5, 3.5);
        assert_eq!(c.local_performer.x, 1.5);
        if let SessionMessage::StateUpdate { performer } = msg {
            assert_eq!(performer.z, 3.5);
        } else {
            panic!("expected StateUpdate");
        }
    }

    // ---- JSON round-trips --------------------------------------------------

    #[test]
    fn serialize_deserialize_join_session() {
        let p   = performer("p1");
        let msg = SessionMessage::JoinSession { session_id: "s1".to_string(), performer: p };
        let json = CollaborationClient::serialize_message(&msg);
        let back = CollaborationClient::deserialize_message(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn serialize_deserialize_leave_session() {
        let msg  = SessionMessage::LeaveSession { performer_id: "p99".to_string() };
        let json = CollaborationClient::serialize_message(&msg);
        let back = CollaborationClient::deserialize_message(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn serialize_deserialize_state_update() {
        let mut p = performer("p2");
        p.x = 3.14;
        let msg  = SessionMessage::StateUpdate { performer: p };
        let json = CollaborationClient::serialize_message(&msg);
        let back = CollaborationClient::deserialize_message(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn serialize_deserialize_parameter_sync() {
        let mut params = HashMap::new();
        params.insert("rho".to_string(), 28.0);
        params.insert("sigma".to_string(), 10.0);
        let msg  = SessionMessage::ParameterSync { params };
        let json = CollaborationClient::serialize_message(&msg);
        let back = CollaborationClient::deserialize_message(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn serialize_deserialize_chat_message() {
        let msg  = SessionMessage::ChatMessage { performer_id: "p1".into(), text: "hi!".into() };
        let json = CollaborationClient::serialize_message(&msg);
        let back = CollaborationClient::deserialize_message(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn serialize_deserialize_kickoff() {
        let msg  = SessionMessage::KickOff { session_id: "room1".into(), bpm: 128.0 };
        let json = CollaborationClient::serialize_message(&msg);
        let back = CollaborationClient::deserialize_message(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn deserialize_invalid_json_returns_err() {
        assert!(CollaborationClient::deserialize_message("not json at all").is_err());
    }

    #[test]
    fn json_type_tag_present() {
        let msg  = SessionMessage::KickOff { session_id: "x".into(), bpm: 100.0 };
        let json = CollaborationClient::serialize_message(&msg);
        assert!(json.contains(r#""type":"KickOff""#));
    }

    #[test]
    fn performer_color_serializes() {
        let mut p = performer("p1");
        p.color = [255, 128, 0];
        let msg  = SessionMessage::StateUpdate { performer: p };
        let json = CollaborationClient::serialize_message(&msg);
        let back = CollaborationClient::deserialize_message(&json).unwrap();
        if let SessionMessage::StateUpdate { performer } = back {
            assert_eq!(performer.color, [255, 128, 0]);
        }
    }
}
