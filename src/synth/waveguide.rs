/// Digital waveguide string model (Karplus-Strong extended).
/// Two delay lines for forward/backward traveling waves.
pub struct WaveguideString {
    delay_fwd: Vec<f32>,
    delay_bck: Vec<f32>,
    fwd_pos: usize,
    bck_pos: usize,
    sample_rate: f32,
    pub tension: f32,   // 0..1 -> frequency scaling
    pub damping: f32,   // 0..1 -> loss per reflection
    pub length: f32,    // delay line length in samples -> base frequency
    pub excite: bool,   // trigger a pluck excitation
    pub excite_pos: f32, // 0..1 excitation position
    noise_seed: u64,
}

const MAX_DELAY: usize = 4096;

impl WaveguideString {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            delay_fwd: vec![0.0; MAX_DELAY],
            delay_bck: vec![0.0; MAX_DELAY],
            fwd_pos: 0,
            bck_pos: 0,
            sample_rate,
            tension: 0.5,
            damping: 0.98,
            length: (sample_rate / 220.0).clamp(2.0, MAX_DELAY as f32),
            excite: false,
            excite_pos: 0.5,
            noise_seed: 1234567,
        }
    }

    pub fn set_freq(&mut self, hz: f32) {
        let hz = hz.max(10.0);
        let scaled = hz * (0.5 + self.tension * 1.5);
        self.length = (self.sample_rate / scaled).clamp(2.0, MAX_DELAY as f32);
    }

    fn lcg_noise(&mut self) -> f32 {
        self.noise_seed = self.noise_seed.wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.noise_seed >> 33) as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    pub fn next_sample(&mut self) -> f32 {
        let len = (self.length as usize).max(2).min(MAX_DELAY);

        // If excite is requested, inject noise burst at excite_pos
        if self.excite {
            let inject_at = (self.excite_pos * len as f32) as usize % len;
            for k in 0..len {
                let n = self.lcg_noise();
                self.delay_fwd[(inject_at + k) % len] = n;
                self.delay_bck[(inject_at + k) % len] = n;
            }
            self.fwd_pos = 0;
            self.bck_pos = 0;
            self.excite = false;
        }

        // Read samples from both lines
        let fwd_out = self.delay_fwd[self.fwd_pos % len];
        let bck_idx = (self.bck_pos + len - 1) % len;
        let bck_out = self.delay_bck[bck_idx];

        // Mix at reflection with damping
        let mixed = (fwd_out + bck_out) * 0.5 * self.damping;

        // Write back
        self.delay_fwd[self.fwd_pos % len] = mixed;
        self.delay_bck[self.bck_pos % len] = -mixed; // reflection inverts phase

        // Advance positions
        self.fwd_pos = (self.fwd_pos + 1) % len;
        self.bck_pos = (self.bck_pos + 1) % len;

        fwd_out
    }
}
