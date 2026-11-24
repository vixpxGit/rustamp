use rustfft::{num_complex::Complex, FftPlanner};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub const NUM_BANDS: usize = 19;
pub const BUFFER_SIZE: usize = 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum VisMode {
    Spectrum,
    Oscilloscope,
    Off,
}

pub struct VisualizerData {
    pub samples: VecDeque<f32>,
    pub sample_rate: u32,
    pub is_active: bool,
}

impl VisualizerData {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(BUFFER_SIZE * 2),
            sample_rate: 44100,
            is_active: false,
        }
    }

    pub fn push_sample(&mut self, s: f32) {
        if self.samples.len() >= BUFFER_SIZE {
            self.samples.pop_front();
        }
        self.samples.push_back(s);
    }
}

pub struct VisualizerState {
    pub mode: VisMode,
    pub bands: [f32; NUM_BANDS],
    pub peaks: [f32; NUM_BANDS],
    pub peak_fall: [f32; NUM_BANDS],
    pub osc_samples: Vec<f32>,
    pub data: Arc<Mutex<VisualizerData>>,
    hann_window: Vec<f32>,
    planner: FftPlanner<f32>,
}

impl VisualizerState {
    pub fn new(data: Arc<Mutex<VisualizerData>>) -> Self {
        let mut hann_window = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            let mult = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / BUFFER_SIZE as f32).cos());
            hann_window.push(mult);
        }

        Self {
            mode: VisMode::Spectrum,
            bands: [0.0; NUM_BANDS],
            peaks: [0.0; NUM_BANDS],
            peak_fall: [0.0; NUM_BANDS],
            osc_samples: Vec::with_capacity(BUFFER_SIZE),
            data,
            hann_window,
            planner: FftPlanner::new(),
        }
    }

    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            VisMode::Spectrum => VisMode::Oscilloscope,
            VisMode::Oscilloscope => VisMode::Off,
            VisMode::Off => VisMode::Spectrum,
        };
    }

    pub fn update(&mut self) {
        let (raw_samples, sample_rate, is_active) = {
            if let Ok(guard) = self.data.lock() {
                let v: Vec<f32> = guard.samples.iter().copied().collect();
                (v, guard.sample_rate, guard.is_active)
            } else {
                return;
            }
        };

        if !is_active || raw_samples.is_empty() {
            // Decay bars to zero smoothly when inactive
            for i in 0..NUM_BANDS {
                self.bands[i] = (self.bands[i] * 0.85).max(0.0);
                self.peaks[i] = (self.peaks[i] - 0.03).max(0.0);
            }
            self.osc_samples.clear();
            return;
        }

        // Copy for oscilloscope
        self.osc_samples = raw_samples.clone();

        if self.mode == VisMode::Spectrum && raw_samples.len() >= 256 {
            let n = BUFFER_SIZE.min(raw_samples.len());
            let fft = self.planner.plan_fft_forward(n);

            let mut buffer: Vec<Complex<f32>> = (0..n)
                .map(|i| {
                    let s = raw_samples.get(raw_samples.len() - n + i).copied().unwrap_or(0.0);
                    let win = self.hann_window.get(i).copied().unwrap_or(1.0);
                    Complex::new(s * win, 0.0)
                })
                .collect();

            fft.process(&mut buffer);

            // Map FFT bins to 19 logarithmic bands
            // Band centers roughly: 50, 80, 125, 200, 315, 500, 800, 1.25k, 2k, 3.15k, 5k, 8k, 10k, 12k, 14k, 16k, 18k, 20k, 22k
            let nyquist = sample_rate as f32 / 2.0;
            let bin_hz = nyquist / (n / 2) as f32;

            let freq_edges = [
                30.0, 70.0, 110.0, 160.0, 250.0, 400.0, 650.0, 1000.0,
                1600.0, 2500.0, 4000.0, 6000.0, 8000.0, 10500.0, 13000.0,
                15500.0, 17500.0, 19500.0, 21500.0, 24000.0,
            ];

            for band in 0..NUM_BANDS {
                let f_low = freq_edges[band];
                let f_high = freq_edges[band + 1];

                let bin_low = ((f_low / bin_hz) as usize).clamp(1, n / 2 - 1);
                let bin_high = ((f_high / bin_hz) as usize).clamp(bin_low + 1, n / 2);

                let mut sum = 0.0;
                let count = (bin_high - bin_low).max(1);
                for k in bin_low..bin_high {
                    let mag = buffer[k].norm() / (n as f32 * 0.25);
                    sum += mag;
                }
                let avg = sum / (count as f32);

                // Logarithmic compression for perceptual scaling
                // Boost high frequencies slightly because musical energy drops off with 1/f
                let hf_boost = 1.0 + (band as f32 / NUM_BANDS as f32) * 1.8;
                let target_val = (avg * 8.0 * hf_boost).clamp(0.0, 1.0);

                // Smooth attack and decay
                if target_val > self.bands[band] {
                    self.bands[band] = target_val;
                } else {
                    self.bands[band] = (self.bands[band] * 0.75 + target_val * 0.25).max(0.0);
                }

                // Winamp peak hold and drop
                if self.bands[band] >= self.peaks[band] {
                    self.peaks[band] = self.bands[band];
                    self.peak_fall[band] = 0.005;
                } else {
                    self.peak_fall[band] += 0.004; // Accelerating gravity drop
                    self.peaks[band] = (self.peaks[band] - self.peak_fall[band]).max(self.bands[band]);
                }
            }
        }
    }
}
