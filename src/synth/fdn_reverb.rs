/// FDN Reverb — 8-channel Feedback Delay Network.
///
/// Better than Freeverb: denser echo density, smoother tail, less metallic
/// coloration. Uses a Hadamard 8x8 mixing matrix applied each sample via a
/// fast Walsh-Hadamard transform (24 ops instead of 64). Each channel has
/// an independent first-order lowpass for high-frequency damping.

// Delay lengths in samples at 44100 Hz (coprime, spread across RT60 range).
// Scaled proportionally for other sample rates.
const FDN_DELAYS_44K: [usize; 8] = [1559, 1877, 2053, 2381, 2713, 3067, 3413, 3761];

struct FdnChannel {
    buf: Vec<f32>,
    pos: usize,
    lp_state: f32, // first-order lowpass state
}

impl FdnChannel {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], pos: 0, lp_state: 0.0 }
    }

    fn read(&self) -> f32 {
        self.buf[self.pos]
    }

    fn write_and_advance(&mut self, val: f32) {
        self.buf[self.pos] = val;
        self.pos = (self.pos + 1) % self.buf.len();
    }

    fn apply_damping(&mut self, damp: f32) -> f32 {
        // First-order lowpass: y[n] = (1-d)*x[n] + d*y[n-1]
        self.lp_state = (1.0 - damp) * self.read() + damp * self.lp_state;
        if !self.lp_state.is_finite() { self.lp_state = 0.0; }
        self.lp_state
    }
}

/// Fast Walsh-Hadamard transform for N=8 (in-place).
/// Normalizes by 1/sqrt(8) so the matrix is unitary (lossless mixing).
fn hadamard8(v: &mut [f32; 8]) {
    // Stage 1: stride-1 butterflies
    for i in (0..8).step_by(2) {
        let a = v[i]; let b = v[i + 1];
        v[i] = a + b; v[i + 1] = a - b;
    }
    // Stage 2: stride-2 butterflies
    for base in [0usize, 4] {
        for j in 0..2 {
            let a = v[base + j]; let b = v[base + j + 2];
            v[base + j] = a + b; v[base + j + 2] = a - b;
        }
    }
    // Stage 3: stride-4 butterflies
    for j in 0..4 {
        let a = v[j]; let b = v[j + 4];
        v[j] = a + b; v[j + 4] = a - b;
    }
    // Normalize
    let norm = (8.0f32).sqrt().recip();
    for x in v.iter_mut() { *x *= norm; }
}

pub struct FdnReverb {
    channels: Vec<FdnChannel>,
    pub feedback: f32,
    pub damping: f32,
    pub wet: f32,
}

impl FdnReverb {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 44100.0;
        let channels = FDN_DELAYS_44K
            .iter()
            .map(|&d| FdnChannel::new((d as f32 * scale) as usize))
            .collect();
        Self { channels, feedback: 0.82, damping: 0.25, wet: 0.4 }
    }

    /// Process one stereo sample pair. Returns (left_out, right_out).
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let input_l = if input_l.is_finite() { input_l } else { 0.0 };
        let input_r = if input_r.is_finite() { input_r } else { 0.0 };
        let input_mono = (input_l + input_r) * 0.01; // scale to prevent saturation

        // Read output from all 8 delay lines, apply damping lowpass
        let mut state = [0.0f32; 8];
        for (i, ch) in self.channels.iter_mut().enumerate() {
            state[i] = ch.apply_damping(self.damping);
        }

        // Apply Hadamard mixing (diffusion matrix)
        hadamard8(&mut state);

        // Write fed-back signals + input to each delay line
        // Alternate input injection (L to even channels, R to odd) for stereo image
        for (i, ch) in self.channels.iter_mut().enumerate() {
            let excitation = if i % 2 == 0 { input_l } else { input_r };
            let write_val = (state[i] * self.feedback + excitation * 0.1).clamp(-10.0, 10.0);
            // mono input blended in
            let _ = input_mono; // used via excitation
            ch.write_and_advance(write_val + input_mono);
        }

        // Build stereo output: sum even channels to left, odd to right
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        for (i, s) in state.iter().enumerate() {
            if i % 2 == 0 { out_l += s; } else { out_r += s; }
        }
        // Normalize (4 channels per side)
        out_l *= 0.25;
        out_r *= 0.25;

        if !out_l.is_finite() || !out_r.is_finite() {
            // Reset on NaN/inf
            for ch in &mut self.channels {
                ch.buf.iter_mut().for_each(|x| *x = 0.0);
                ch.lp_state = 0.0;
            }
            return (input_l * (1.0 - self.wet), input_r * (1.0 - self.wet));
        }

        let dry = 1.0 - self.wet;
        (input_l * dry + out_l * self.wet, input_r * dry + out_r * self.wet)
    }
}
