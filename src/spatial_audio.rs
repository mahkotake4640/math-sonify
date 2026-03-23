//! 3D spatial audio with HRTF approximation and room acoustics.

/// A point in 3D space.
#[derive(Debug, Clone, Copy)]
pub struct Position3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Position3D {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn distance(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Azimuth in degrees (horizontal angle, XY plane).
    pub fn azimuth_deg(&self, other: &Self) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        dy.atan2(dx).to_degrees()
    }

    /// Elevation in degrees (vertical angle).
    pub fn elevation_deg(&self, other: &Self) -> f64 {
        let dist = self.distance(other);
        if dist < 1e-9 {
            return 0.0;
        }
        let dz = other.z - self.z;
        (dz / dist).asin().to_degrees()
    }
}

/// Panning law selection.
#[derive(Debug, Clone)]
pub enum PanLaw {
    Linear,
    EqualPower,
    /// Constant power (-3 dB crossover).
    ConstantPower,
}

/// Pan a mono signal to stereo based on azimuth (degrees, -180..180).
/// Azimuth -90 = hard left, +90 = hard right.
pub fn pan_signal(mono: &[f32], azimuth_deg: f64, law: &PanLaw) -> (Vec<f32>, Vec<f32>) {
    // Map azimuth to pan ∈ [-1, 1]
    let pan = (azimuth_deg / 180.0).clamp(-1.0, 1.0);
    let (l_gain, r_gain) = match law {
        PanLaw::Linear => {
            let r = ((pan + 1.0) / 2.0) as f32;
            (1.0 - r, r)
        }
        PanLaw::EqualPower => {
            let angle = (pan + 1.0) / 2.0 * std::f64::consts::FRAC_PI_2;
            (angle.cos() as f32, angle.sin() as f32)
        }
        PanLaw::ConstantPower => {
            // Same as equal power at -3 dB crossover
            let angle = (pan + 1.0) / 2.0 * std::f64::consts::FRAC_PI_2;
            (angle.cos() as f32, angle.sin() as f32)
        }
    };
    let left: Vec<f32> = mono.iter().map(|s| s * l_gain).collect();
    let right: Vec<f32> = mono.iter().map(|s| s * r_gain).collect();
    (left, right)
}

/// Inverse-square-law distance attenuation.
pub fn distance_attenuation(distance: f64, reference_distance: f64) -> f64 {
    let d = distance.max(reference_distance);
    1.0 / (d * d)
}

/// Simplified HRTF using azimuth bins with ITD and gain.
pub struct HrtfApproximation {
    /// Azimuth bin centers in degrees (0..360).
    pub azimuth_bins: Vec<f64>,
    /// Interaural time delay for left ear per bin (seconds).
    pub left_delays: Vec<f64>,
    /// Interaural time delay for right ear per bin (seconds).
    pub right_delays: Vec<f64>,
    pub left_gains: Vec<f64>,
    pub right_gains: Vec<f64>,
}

impl HrtfApproximation {
    /// 8-bin HRTF with plausible ITD (max 0.7ms at 90°).
    pub fn default_hrtf() -> Self {
        // Bins at 0, 45, 90, 135, 180, 225, 270, 315 degrees
        let azimuth_bins = vec![0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0];
        // ITD: 0.7ms max at 90° (source to listener's right)
        // left delay is longer when source is to the right
        let max_itd = 0.0007_f64; // seconds
        let left_delays = vec![
            0.0,         // 0°: front
            0.0,         // 45°: slight right
            0.0,         // 90°: hard right — right ear first
            0.0,         // 135°: back right
            0.0,         // 180°: back
            max_itd,     // 225°: back left
            max_itd,     // 270°: hard left — left ear first
            max_itd / 2.0, // 315°: slight left
        ];
        let right_delays = vec![
            0.0,
            max_itd / 2.0,
            max_itd,
            max_itd,
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        // Gains: louder in the nearer ear
        let left_gains = vec![0.9, 0.7, 0.4, 0.5, 0.8, 1.0, 1.0, 0.95];
        let right_gains = vec![0.9, 1.0, 1.0, 0.95, 0.8, 0.5, 0.4, 0.7];

        Self {
            azimuth_bins,
            left_delays,
            right_delays,
            left_gains,
            right_gains,
        }
    }

    /// Spatialize mono signal at given azimuth (degrees).
    pub fn spatialize(
        &self,
        mono: &[f32],
        azimuth_deg: f64,
        sample_rate: f64,
    ) -> (Vec<f32>, Vec<f32>) {
        // Normalize azimuth to [0, 360)
        let az = ((azimuth_deg % 360.0) + 360.0) % 360.0;

        // Find nearest two bins and interpolate
        let _n = self.azimuth_bins.len();
        let mut best = 0;
        let mut min_dist = f64::MAX;
        for (i, &bin) in self.azimuth_bins.iter().enumerate() {
            let d = (az - bin).abs().min(360.0 - (az - bin).abs());
            if d < min_dist {
                min_dist = d;
                best = i;
            }
        }

        let l_gain = self.left_gains[best] as f32;
        let r_gain = self.right_gains[best] as f32;
        let l_delay_smp = (self.left_delays[best] * sample_rate).round() as usize;
        let r_delay_smp = (self.right_delays[best] * sample_rate).round() as usize;

        let apply_delay = |signal: &[f32], delay: usize, gain: f32| -> Vec<f32> {
            let mut out = vec![0.0f32; signal.len() + delay];
            for (i, &s) in signal.iter().enumerate() {
                out[i + delay] = s * gain;
            }
            out.truncate(signal.len());
            out
        };

        let left = apply_delay(mono, l_delay_smp, l_gain);
        let right = apply_delay(mono, r_delay_smp, r_gain);
        (left, right)
    }
}

/// Simple room reverb using early reflections.
pub struct RoomReverb {
    pub room_size: f64,
    pub damping: f64,
    /// (delay_samples, gain) pairs.
    pub early_reflections: Vec<(usize, f64)>,
}

impl RoomReverb {
    pub fn new(room_size: f64, damping: f64, sample_rate: f64) -> Self {
        // Generate 6 early reflections at room-size-dependent delays
        let base_ms = room_size * 50.0; // scale room_size [0,1] to ~50ms max
        let reflections: Vec<(usize, f64)> = (1..=6)
            .map(|i| {
                let delay_ms = base_ms * i as f64 / 3.0;
                let delay_smp = (delay_ms * sample_rate / 1000.0).round() as usize;
                let gain = (1.0 - damping) * (1.0 - i as f64 * 0.12);
                (delay_smp.max(1), gain.max(0.0))
            })
            .collect();

        Self {
            room_size,
            damping,
            early_reflections: reflections,
        }
    }

    /// Add early reflections to signal.
    pub fn process(&self, signal: &[f32]) -> Vec<f32> {
        let max_delay = self
            .early_reflections
            .iter()
            .map(|(d, _)| *d)
            .max()
            .unwrap_or(0);
        let out_len = signal.len() + max_delay;
        let mut output = vec![0.0f32; out_len];

        // Copy dry signal
        for (i, &s) in signal.iter().enumerate() {
            output[i] += s;
        }

        // Add reflections
        for &(delay, gain) in &self.early_reflections {
            for (i, &s) in signal.iter().enumerate() {
                output[i + delay] += s * gain as f32;
            }
        }

        output
    }
}

/// A 3D audio scene with a listener and multiple sources.
pub struct SpatialScene {
    pub listener: Position3D,
    pub sources: Vec<(Position3D, Vec<f32>)>,
}

impl SpatialScene {
    pub fn new(listener: Position3D) -> Self {
        Self {
            listener,
            sources: Vec::new(),
        }
    }

    pub fn add_source(&mut self, pos: Position3D, audio: Vec<f32>) {
        self.sources.push((pos, audio));
    }

    /// Mix all sources into a stereo output.
    pub fn mix(&self, sample_rate: f64) -> (Vec<f32>, Vec<f32>) {
        let hrtf = HrtfApproximation::default_hrtf();
        let max_len = self.sources.iter().map(|(_, a)| a.len()).max().unwrap_or(0);

        let mut left_mix = vec![0.0f32; max_len];
        let mut right_mix = vec![0.0f32; max_len];

        for (pos, audio) in &self.sources {
            let dist = self.listener.distance(pos);
            let atten = distance_attenuation(dist, 1.0) as f32;
            let az = self.listener.azimuth_deg(pos);

            let attenuated: Vec<f32> = audio.iter().map(|&s| s * atten).collect();
            let (l, r) = hrtf.spatialize(&attenuated, az, sample_rate);

            for (i, (&ls, &rs)) in l.iter().zip(r.iter()).enumerate() {
                if i < left_mix.len() {
                    left_mix[i] += ls;
                    right_mix[i] += rs;
                }
            }
        }

        (left_mix, right_mix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pan_left_channel_louder_at_negative_90() {
        let mono: Vec<f32> = vec![1.0; 100];
        let (left, right) = pan_signal(&mono, -90.0, &PanLaw::EqualPower);
        let l_sum: f32 = left.iter().sum();
        let r_sum: f32 = right.iter().sum();
        assert!(l_sum > r_sum, "left should be louder at -90°, l={l_sum}, r={r_sum}");
    }

    #[test]
    fn test_distance_attenuation_decreases_with_distance() {
        let a1 = distance_attenuation(2.0, 1.0);
        let a2 = distance_attenuation(4.0, 1.0);
        assert!(a1 > a2, "attenuation should decrease with distance");
    }

    #[test]
    fn test_hrtf_returns_stereo() {
        let hrtf = HrtfApproximation::default_hrtf();
        let mono: Vec<f32> = vec![0.5; 100];
        let (l, r) = hrtf.spatialize(&mono, 90.0, 44100.0);
        assert_eq!(l.len(), 100);
        assert_eq!(r.len(), 100);
    }

    #[test]
    fn test_reverb_adds_length() {
        let reverb = RoomReverb::new(1.0, 0.3, 44100.0);
        let signal: Vec<f32> = vec![1.0; 1000];
        let output = reverb.process(&signal);
        assert!(output.len() > signal.len(), "reverb should extend signal length");
    }

    #[test]
    fn test_3d_position_azimuth_and_elevation() {
        let listener = Position3D::new(0.0, 0.0, 0.0);
        let source_right = Position3D::new(1.0, 0.0, 0.0);
        let az = listener.azimuth_deg(&source_right);
        assert!((az - 0.0).abs() < 1.0, "azimuth to right should be ~0°, got {az}");

        let source_above = Position3D::new(0.0, 0.0, 1.0);
        let el = listener.elevation_deg(&source_above);
        assert!((el - 90.0).abs() < 1.0, "elevation above should be ~90°, got {el}");
    }
}
