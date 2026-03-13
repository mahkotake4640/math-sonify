/// Second-order biquad filter (transposed direct form II).
/// Supports low-pass and band-pass configurations.
#[derive(Clone)]
pub struct BiquadFilter {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    z1: f32, z2: f32,
}

impl BiquadFilter {
    pub fn low_pass(cutoff_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = std::f32::consts::TAU * cutoff_hz / sample_rate;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 - cos_w0) / 2.0 / a0,
            b1: (1.0 - cos_w0) / a0,
            b2: (1.0 - cos_w0) / 2.0 / a0,
            a1: -2.0 * cos_w0 / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0, z2: 0.0,
        }
    }

    pub fn band_pass(center_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = std::f32::consts::TAU * center_hz / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b1: 0.0,
            b2: -alpha / a0,
            a1: -2.0 * w0.cos() / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0, z2: 0.0,
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    pub fn update_lp(&mut self, cutoff_hz: f32, q: f32, sample_rate: f32) {
        let new = Self::low_pass(cutoff_hz, q, sample_rate);
        self.b0 = new.b0; self.b1 = new.b1; self.b2 = new.b2;
        self.a1 = new.a1; self.a2 = new.a2;
    }
}
