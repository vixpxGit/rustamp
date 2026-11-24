/// Standard Biquad Filter (Peaking EQ) based on Robert Bristow-Johnson's Audio EQ Cookbook
#[derive(Clone, Debug)]
pub struct BiquadFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    // Filter state for left and right channels
    x1: [f32; 2],
    x2: [f32; 2],
    y1: [f32; 2],
    y2: [f32; 2],
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
    pub sample_rate: f32,
}

impl BiquadFilter {
    pub fn new_peaking(freq: f32, gain_db: f32, q: f32, sample_rate: f32) -> Self {
        let mut filter = Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: [0.0; 2],
            x2: [0.0; 2],
            y1: [0.0; 2],
            y2: [0.0; 2],
            freq,
            gain_db,
            q,
            sample_rate,
        };
        filter.recompute();
        filter
    }

    pub fn set_gain(&mut self, gain_db: f32) {
        if (self.gain_db - gain_db).abs() > 0.01 {
            self.gain_db = gain_db;
            self.recompute();
        }
    }

    #[allow(dead_code)]
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if (self.sample_rate - sample_rate).abs() > 1.0 {
            self.sample_rate = sample_rate;
            self.recompute();
        }
    }

    fn recompute(&mut self) {
        // Clamp frequency to below Nyquist
        let nyquist = self.sample_rate * 0.49;
        let f0 = self.freq.clamp(20.0, nyquist);
        let a = 10.0f32.powf(self.gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * f0 / self.sample_rate;
        let alpha = w0.sin() / (2.0 * self.q);
        let cos_w0 = w0.cos();

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32, channel: usize) -> f32 {
        let ch = channel % 2;
        if self.gain_db.abs() < 0.05 {
            return input; // Bypass when flat
        }
        let out = self.b0 * input + self.b1 * self.x1[ch] + self.b2 * self.x2[ch]
            - self.a1 * self.y1[ch] - self.a2 * self.y2[ch];

        self.x2[ch] = self.x1[ch];
        self.x1[ch] = input;
        self.y2[ch] = self.y1[ch];
        // Anti-denormal protection
        self.y1[ch] = if out.abs() < 1e-15 { 0.0 } else { out };

        out
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.x1 = [0.0; 2];
        self.x2 = [0.0; 2];
        self.y1 = [0.0; 2];
        self.y2 = [0.0; 2];
    }
}
