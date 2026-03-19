use super::DynamicalSystem;

/// Discrete-time delay map: x[n+1] = r * x[n] * (1 - x[n-tau])
///
/// Uses a ring buffer of length tau+1 to store history.
/// The `step(dt)` method advances one discrete step regardless of dt.
pub struct DelayedMap {
    /// Ring buffer holding x history; length = tau + 1
    history: Vec<f64>,
    /// Current write position in the ring buffer
    head: usize,
    pub r: f64,
    pub tau: usize,
    /// Exposed state: [x_current, x_delayed]
    state: Vec<f64>,
    speed: f64,
}

impl DelayedMap {
    pub fn new(r: f64, tau: usize) -> Self {
        let buf_len = tau + 1;
        // Initialize all history to 0.5 (interior of [0,1])
        let history = vec![0.5; buf_len];
        let state = vec![0.5, 0.5];
        Self {
            history,
            head: 0,
            r,
            tau,
            state,
            speed: 0.0,
        }
    }

    fn current(&self) -> f64 {
        self.history[self.head]
    }

    fn delayed(&self) -> f64 {
        let buf_len = self.history.len();
        let delayed_idx = (self.head + buf_len - self.tau) % buf_len;
        self.history[delayed_idx]
    }
}

impl DynamicalSystem for DelayedMap {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn dimension(&self) -> usize {
        2
    }

    fn name(&self) -> &str {
        "Delayed Map"
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    fn set_state(&mut self, s: &[f64]) {
        // Reset entire history to the provided x value (or 0.5 if not given)
        let x = if !s.is_empty() && s[0].is_finite() { s[0] } else { 0.5 };
        for v in &mut self.history {
            *v = x;
        }
        self.head = 0;
        self.state[0] = x;
        self.state[1] = x;
    }

    fn step(&mut self, _dt: f64) {
        let x_cur = self.current();
        let x_del = self.delayed();
        let x_next = self.r * x_cur * (1.0 - x_del);

        // Advance head
        let buf_len = self.history.len();
        self.head = (self.head + 1) % buf_len;
        self.history[self.head] = x_next;

        let prev_x = self.state[0];
        self.state[0] = x_next;
        self.state[1] = self.delayed();
        self.speed = (x_next - prev_x).abs();
    }
}
