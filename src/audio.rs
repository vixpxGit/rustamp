use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use serde::{Deserialize, Serialize};

use crate::biquad::BiquadFilter;
use crate::visualizer::VisualizerData;

pub const EQ_FREQUENCIES: [f32; 10] = [
    60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0,
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub title: String,
    pub path: Option<PathBuf>,
    pub duration_secs: f32,
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate_kbps: u32,
    pub is_demo: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

pub struct EqualizerState {
    pub enabled: bool,
    pub preamp_db: f32,
    pub bands_db: [f32; 10],
}

impl Default for EqualizerState {
    fn default() -> Self {
        Self {
            enabled: true,
            preamp_db: 0.0,
            bands_db: [0.0; 10],
        }
    }
}

pub struct AudioEngine {
    _stream: Option<OutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    pub vis_data: Arc<Mutex<VisualizerData>>,
    pub eq_state: Arc<Mutex<EqualizerState>>,
    pub playlist: Vec<Track>,
    pub current_index: Option<usize>,
    pub state: PlaybackState,
    pub volume: f32,
    pub balance: f32,
    balance_bits: Arc<AtomicU32>,
    pub elapsed_secs: f32,
    pub shuffle: bool,
    pub repeat: bool,
    play_start_instant: Option<std::time::Instant>,
    accumulated_secs: f32,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (stream, stream_handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(e) => {
                eprintln!("Warning: Failed to open default audio output stream: {}", e);
                (None, None)
            }
        };

        let vis_data = Arc::new(Mutex::new(VisualizerData::new()));
        let eq_state = Arc::new(Mutex::new(EqualizerState::default()));

        let mut engine = Self {
            _stream: stream,
            stream_handle,
            sink: None,
            vis_data,
            eq_state,
            playlist: Vec::new(),
            current_index: None,
            state: PlaybackState::Stopped,
            volume: 0.8,
            balance: 0.0,
            balance_bits: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            elapsed_secs: 0.0,
            shuffle: false,
            repeat: false,
            play_start_instant: None,
            accumulated_secs: 0.0,
        };

        // Add built-in demo tracks
        engine.add_demo_tracks();
        engine
    }

    pub fn add_demo_tracks(&mut self) {
        self.playlist.push(Track {
            title: "DJ Mike Llama - Llama Whippin' Intro".to_string(),
            path: None,
            duration_secs: 18.0,
            sample_rate: 44100,
            channels: 2,
            bitrate_kbps: 192,
            is_demo: true,
        });
        self.playlist.push(Track {
            title: "Nullsoft Sound Lab - 90s Neon Chiptune".to_string(),
            path: None,
            duration_secs: 32.0,
            sample_rate: 44100,
            channels: 2,
            bitrate_kbps: 128,
            is_demo: true,
        });
        if self.current_index.is_none() {
            self.current_index = Some(0);
        }
    }

    pub fn add_file<P: AsRef<Path>>(&mut self, path: P) -> Option<usize> {
        let path_buf = path.as_ref().to_path_buf();
        let file_name = path_buf
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown Track")
            .to_string();

        let track = Track {
            title: file_name,
            path: Some(path_buf),
            duration_secs: 0.0, // Updated once probed or played
            sample_rate: 44100,
            channels: 2,
            bitrate_kbps: 320,
            is_demo: false,
        };

        self.playlist.push(track);
        let idx = self.playlist.len() - 1;
        if self.current_index.is_none() {
            self.current_index = Some(idx);
        }
        Some(idx)
    }

    pub fn play(&mut self) {
        match self.state {
            PlaybackState::Paused => {
                if let Some(ref sink) = self.sink {
                    sink.play();
                }
                self.state = PlaybackState::Playing;
                self.play_start_instant = Some(std::time::Instant::now());
                if let Ok(mut v) = self.vis_data.lock() {
                    v.is_active = true;
                }
            }
            PlaybackState::Stopped => {
                if let Some(idx) = self.current_index {
                    self.start_track(idx, 0.0);
                } else if !self.playlist.is_empty() {
                    self.current_index = Some(0);
                    self.start_track(0, 0.0);
                }
            }
            PlaybackState::Playing => {
                // Restart or continue
            }
        }
    }

    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            if let Some(ref sink) = self.sink {
                sink.pause();
            }
            self.state = PlaybackState::Paused;
            if let Some(start) = self.play_start_instant {
                self.accumulated_secs += start.elapsed().as_secs_f32();
                self.play_start_instant = None;
            }
            self.elapsed_secs = self.accumulated_secs;
            if let Ok(mut v) = self.vis_data.lock() {
                v.is_active = false;
            }
        } else if self.state == PlaybackState::Paused {
            self.play();
        }
    }

    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.state = PlaybackState::Stopped;
        self.play_start_instant = None;
        self.accumulated_secs = 0.0;
        self.elapsed_secs = 0.0;
        if let Ok(mut v) = self.vis_data.lock() {
            v.is_active = false;
            v.samples.clear();
        }
    }

    pub fn next_track(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        let next_idx = if self.shuffle {
            rand_idx(self.playlist.len(), self.current_index.unwrap_or(0))
        } else {
            let cur = self.current_index.unwrap_or(0);
            (cur + 1) % self.playlist.len()
        };
        self.current_index = Some(next_idx);
        self.start_track(next_idx, 0.0);
    }

    pub fn prev_track(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        // If elapsed > 3 seconds, restart current track like Winamp
        if self.elapsed_secs > 3.0 {
            if let Some(idx) = self.current_index {
                self.start_track(idx, 0.0);
                return;
            }
        }
        let prev_idx = {
            let cur = self.current_index.unwrap_or(0);
            if cur == 0 {
                self.playlist.len().saturating_sub(1)
            } else {
                cur - 1
            }
        };
        self.current_index = Some(prev_idx);
        self.start_track(prev_idx, 0.0);
    }

    pub fn seek(&mut self, target_secs: f32) {
        if let Some(idx) = self.current_index {
            let track = &self.playlist[idx];
            let clamped = target_secs.clamp(0.0, track.duration_secs.max(1.0));
            self.start_track(idx, clamped);
        }
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        if let Some(ref sink) = self.sink {
            sink.set_volume(self.volume);
        }
    }

    pub fn set_balance(&mut self, bal: f32) {
        self.balance = bal.clamp(-1.0, 1.0);
        self.balance_bits
            .store(self.balance.to_bits(), Ordering::Relaxed);
    }

    pub fn update(&mut self) {
        if self.state == PlaybackState::Playing {
            if let Some(start) = self.play_start_instant {
                self.elapsed_secs = self.accumulated_secs + start.elapsed().as_secs_f32();
            }

            // Check if track ended
            let current_duration = self
                .current_index
                .and_then(|idx| self.playlist.get(idx))
                .map(|t| t.duration_secs)
                .unwrap_or(0.0);

            let sink_empty = self.sink.as_ref().map(|s| s.empty()).unwrap_or(true);

            if sink_empty || (current_duration > 0.0 && self.elapsed_secs >= current_duration) {
                if self.repeat {
                    if let Some(idx) = self.current_index {
                        self.start_track(idx, 0.0);
                    }
                } else if self.playlist.len() > 1 {
                    self.next_track();
                } else {
                    self.stop();
                }
            }
        }
    }

    fn start_track(&mut self, idx: usize, start_offset: f32) {
        self.stop();
        if idx >= self.playlist.len() {
            return;
        }

        let Some(ref handle) = self.stream_handle else {
            eprintln!("Audio output handle not available");
            return;
        };

        let Ok(sink) = Sink::try_new(handle) else {
            eprintln!("Failed to create audio sink");
            return;
        };

        sink.set_volume(self.volume);

        let track = &mut self.playlist[idx];
        let vis_clone = Arc::clone(&self.vis_data);
        let eq_clone = Arc::clone(&self.eq_state);
        let balance_bits = Arc::clone(&self.balance_bits);

        if track.is_demo {
            // Demo chiptune synthesizer
            let demo_type = idx % 2;
            let synth = WinampDemoSource::new(demo_type, track.duration_secs, start_offset);
            let processed = ProcessedSource::new(synth, vis_clone, eq_clone, balance_bits);
            sink.append(processed);
        } else if let Some(ref path) = track.path {
            match File::open(path) {
                Ok(file) => {
                    let reader = BufReader::new(file);
                    match Decoder::new(reader) {
                        Ok(decoder) => {
                            let sample_rate = decoder.sample_rate();
                            let channels = decoder.channels();
                            track.sample_rate = sample_rate;
                            track.channels = channels;
                            if let Some(dur) = decoder.total_duration() {
                                track.duration_secs = dur.as_secs_f32();
                            }
                            if let Ok(mut v) = self.vis_data.lock() {
                                v.sample_rate = sample_rate;
                            }

                            let source_box: Box<dyn Source<Item = f32> + Send> = if start_offset > 0.0 {
                                Box::new(decoder.skip_duration(Duration::from_secs_f32(start_offset)).convert_samples::<f32>())
                            } else {
                                Box::new(decoder.convert_samples::<f32>())
                            };

                            let processed = ProcessedSource::new(
                                source_box,
                                vis_clone,
                                eq_clone,
                                balance_bits,
                            );
                            sink.append(processed);
                        }
                        Err(e) => {
                            eprintln!("Failed to decode audio file {}: {}", path.display(), e);
                            return;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to open file {}: {}", path.display(), e);
                    return;
                }
            }
        }

        self.sink = Some(sink);
        self.state = PlaybackState::Playing;
        self.accumulated_secs = start_offset;
        self.play_start_instant = Some(std::time::Instant::now());
        self.elapsed_secs = start_offset;
        if let Ok(mut v) = self.vis_data.lock() {
            v.is_active = true;
        }
    }
}

fn rand_idx(len: usize, current: usize) -> usize {
    if len <= 1 {
        return 0;
    }
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(42);
    let mut next = seed % (len - 1);
    if next >= current {
        next += 1;
    }
    next % len
}

/// A wrapper around any rodio Source that:
/// 1. Processes EQ filters and Preamp
/// 2. Applies Left/Right balance
/// 3. Feeds raw audio samples to the Visualizer ring buffer
pub struct ProcessedSource<S> {
    inner: S,
    vis_data: Arc<Mutex<VisualizerData>>,
    eq_state: Arc<Mutex<EqualizerState>>,
    filters: Vec<BiquadFilter>,
    balance_bits: Arc<AtomicU32>,
    channel_toggle: usize,
    sample_rate: u32,
    channels: u16,
}

impl<S> ProcessedSource<S>
where
    S: Source<Item = f32>,
{
    pub fn new(
        inner: S,
        vis_data: Arc<Mutex<VisualizerData>>,
        eq_state: Arc<Mutex<EqualizerState>>,
        balance_bits: Arc<AtomicU32>,
    ) -> Self {
        let sample_rate = inner.sample_rate();
        let channels = inner.channels();

        let mut filters = Vec::with_capacity(10);
        for &freq in &EQ_FREQUENCIES {
            filters.push(BiquadFilter::new_peaking(freq, 0.0, 1.4, sample_rate as f32));
        }

        Self {
            inner,
            vis_data,
            eq_state,
            filters,
            balance_bits,
            channel_toggle: 0,
            sample_rate,
            channels,
        }
    }
}

impl<S> Iterator for ProcessedSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let raw = self.inner.next()?;
        let ch = self.channel_toggle % (self.channels as usize).max(1);
        self.channel_toggle = self.channel_toggle.wrapping_add(1);

        // Fetch EQ state
        let (eq_enabled, preamp, bands) = if let Ok(eq) = self.eq_state.lock() {
            (eq.enabled, eq.preamp_db, eq.bands_db)
        } else {
            (false, 0.0, [0.0; 10])
        };

        let mut sample = raw;

        if eq_enabled {
            // Apply preamp
            let preamp_mult = 10.0f32.powf(preamp / 20.0);
            sample *= preamp_mult;

            // Apply 10 EQ bands
            for (idx, filter) in self.filters.iter_mut().enumerate() {
                filter.set_gain(bands[idx]);
                sample = filter.process_sample(sample, ch);
            }
        }

        // Push to visualizer buffer (pre-balance so panning hard right/left keeps visualizer active)
        if ch == 0 {
            if let Ok(mut vis) = self.vis_data.try_lock() {
                vis.push_sample(sample);
            }
        }

        // Apply Balance in real time
        let balance = f32::from_bits(self.balance_bits.load(Ordering::Relaxed));
        if self.channels == 2 {
            if ch == 0 && balance > 0.0 {
                // Left channel attenuated when balance is to right
                sample *= 1.0 - balance;
            } else if ch == 1 && balance < 0.0 {
                // Right channel attenuated when balance is to left
                sample *= 1.0 + balance;
            }
        }

        Some(sample.clamp(-1.0, 1.0))
    }
}

impl<S> Source for ProcessedSource<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

/// Built-in synthesizer for Winamp Demo Tracks (Fun classic sound)
pub struct WinampDemoSource {
    demo_type: usize,
    sample_rate: u32,
    total_samples: usize,
    current_sample: usize,
    phase1: f32,
    phase2: f32,
}

impl WinampDemoSource {
    pub fn new(demo_type: usize, duration_secs: f32, start_offset: f32) -> Self {
        let sample_rate = 44100;
        let total_samples = (duration_secs * sample_rate as f32 * 2.0) as usize;
        let current_sample = (start_offset * sample_rate as f32 * 2.0) as usize;

        Self {
            demo_type,
            sample_rate,
            total_samples,
            current_sample,
            phase1: 0.0,
            phase2: 0.0,
        }
    }
}

impl Iterator for WinampDemoSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.current_sample >= self.total_samples {
            return None;
        }

        let t = (self.current_sample / 2) as f32 / self.sample_rate as f32;
        let is_left = (self.current_sample % 2) == 0;
        self.current_sample += 1;

        let sample = if self.demo_type == 0 {
            // "Llama Whippin' Intro": Funky 90s FM synth bass + lead melody + kick/snare
            let beat = (t * 2.2) as usize;
            let sub_beat = (t * 8.8) as usize;

            // Chords: Em - G - C - D
            let chords = [
                [164.81, 196.00, 246.94], // Em
                [196.00, 246.94, 293.66], // G
                [130.81, 164.81, 196.00], // C
                [146.83, 185.00, 220.00], // D
            ];
            let chord = &chords[(beat / 4) % 4];

            // Arpeggio notes
            let arp_note = chord[sub_beat % 3];
            let f1 = arp_note;
            let f2 = chord[0] * 0.5; // Bass note

            // Drum emulation
            let beat_frac = (t * 2.2).fract();
            let kick = if beat % 2 == 0 {
                (-beat_frac * 25.0).exp() * (120.0 * (1.0 - beat_frac * 0.8) * 2.0 * std::f32::consts::PI * beat_frac).sin()
            } else {
                0.0
            };
            let snare = if beat % 2 == 1 {
                (-beat_frac * 18.0).exp() * ((self.current_sample as f32 * 0.37).sin() * 0.8)
            } else {
                0.0
            };

            self.phase1 += 2.0 * std::f32::consts::PI * f1 / self.sample_rate as f32;
            self.phase2 += 2.0 * std::f32::consts::PI * f2 / self.sample_rate as f32;

            // Square/saw synth + FM modulation
            let lead = (self.phase1.sin() + 0.5 * (self.phase1 * 2.0).sin()).clamp(-0.8, 0.8) * 0.3;
            let bass = (self.phase2.sin() + 0.3 * (self.phase2 * 3.0).sin()) * 0.4;

            (lead + bass + kick * 0.7 + snare * 0.4) * 0.5
        } else {
            // Neon 90s Chiptune: Upbeat eurodance synth
            let beat = (t * 4.0) as usize;
            let notes = [261.63, 329.63, 392.00, 523.25, 493.88, 392.00, 329.63, 293.66];
            let freq = notes[beat % notes.len()];

            self.phase1 += 2.0 * std::f32::consts::PI * freq / self.sample_rate as f32;
            self.phase2 += 2.0 * std::f32::consts::PI * (freq * 0.5) / self.sample_rate as f32;

            let wave1 = if self.phase1.sin() > 0.0 { 0.25 } else { -0.25 };
            let wave2 = (self.phase2.sin()) * 0.3;

            let kick_frac = (t * 2.0).fract();
            let kick = (-kick_frac * 30.0).exp() * (90.0 * (1.0 - kick_frac) * 2.0 * std::f32::consts::PI * kick_frac).sin();

            (wave1 + wave2 + kick * 0.5) * 0.6
        };

        // Subtle stereo chorus
        let stereo_pan = if is_left { 0.95 } else { 1.05 };
        Some(sample * stereo_pan)
    }
}

impl Source for WinampDemoSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.total_samples as f32 / (self.sample_rate as f32 * 2.0),
        ))
    }
}
