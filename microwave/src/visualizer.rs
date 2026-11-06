use std::f64::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use magnetron::buffer::BufferIndex;
use ringbuf::HeapCons;
use ringbuf::HeapRb;
use ringbuf::traits::Consumer;
use ringbuf::traits::Split;

use crate::tap::AudioRingMessage;
use crate::tap::AudioRingProducer;
use crate::tap::Tap;
use crate::tap::TapContext;

const FFT_SIZE: usize = 2048;

pub(crate) const HISTORY_FRAMES: usize = 8192;
pub(crate) const CAPTURE_CAPACITY_FRAMES: usize = 4 * HISTORY_FRAMES;
const PEAK_WINDOW_FRAMES: usize = 2 * FFT_SIZE;
const MIN_DB: f64 = -120.0;
const STEREO_OUTPUT: usize = usize::MAX;

const CORRELATION_WINDOW_DIVISOR: usize = 8;
const SEARCH_RADIUS_DIVISOR: usize = 8;

const LEVEL_STOPS: [(f32, [f32; 3]); 5] = [
    (0.00, [0.03, 0.04, 0.05]),
    (0.25, [0.00, 0.35, 0.90]),
    (0.50, [0.00, 0.85, 0.70]),
    (0.75, [0.95, 0.75, 0.10]),
    (1.00, [1.00, 0.25, 0.10]),
];

pub(crate) struct VisualizerControl {
    selected_buffer: AtomicUsize,
    enabled: AtomicBool,
}

impl VisualizerControl {
    pub(crate) fn new() -> Self {
        Self {
            selected_buffer: AtomicUsize::new(STEREO_OUTPUT),
            enabled: AtomicBool::new(true),
        }
    }

    pub(crate) fn select(&self, buffer_index: Option<usize>) {
        self.selected_buffer
            .store(buffer_index.unwrap_or(STEREO_OUTPUT), Ordering::Relaxed);
    }

    fn selected(&self) -> Option<usize> {
        match self.selected_buffer.load(Ordering::Relaxed) {
            STEREO_OUTPUT => None,
            index => Some(index),
        }
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

pub(crate) fn create_audio_capture() -> (AudioCaptureWriter, AudioCaptureReader) {
    let (producer, consumer) = HeapRb::<AudioRingMessage>::new(CAPTURE_CAPACITY_FRAMES + 1).split();

    (
        AudioCaptureWriter {
            ring: AudioRingProducer::new(producer),
        },
        AudioCaptureReader {
            consumer,
            samples: Vec::with_capacity(2 * CAPTURE_CAPACITY_FRAMES),
            start_frame: 0,
        },
    )
}

pub(crate) struct AudioCaptureWriter {
    ring: AudioRingProducer,
}

impl AudioCaptureWriter {
    pub(crate) fn push_stereo(&mut self, left: &[f64], right: &[f64]) {
        for (&left, &right) in left.iter().zip(right) {
            if !self.ring.push_frame(left as f32, right as f32) {
                break;
            }
        }
    }

    pub(crate) fn push_mono(&mut self, samples: &[f64]) {
        for &sample in samples {
            let sample = sample as f32;
            if !self.ring.push_frame(sample, sample) {
                break;
            }
        }
    }
}

pub(crate) struct AudioCaptureReader {
    consumer: HeapCons<AudioRingMessage>,
    samples: Vec<f32>,
    // Absolute index of the first retained frame.
    start_frame: u64,
}

impl AudioCaptureReader {
    fn clear(&mut self) {
        let mut discarded = 0;
        while let Some(message) = self.consumer.try_pop() {
            if matches!(message, AudioRingMessage::Frame { .. }) {
                discarded += 1;
            }
        }

        self.start_frame += (self.samples.len() / 2 + discarded) as u64;
        self.samples.clear();
    }

    fn pull(&mut self) -> bool {
        let mut gap = false;

        while let Some(message) = self.consumer.try_pop() {
            match message {
                AudioRingMessage::Frame { left, right } => {
                    self.samples.push(left);
                    self.samples.push(right);
                }
                AudioRingMessage::Overflow => {
                    gap = true;
                    self.start_frame += (self.samples.len() / 2) as u64;
                    self.samples.clear();
                }
                AudioRingMessage::Start | AudioRingMessage::End => {}
            }
        }

        let max_len = 2 * HISTORY_FRAMES;
        if self.samples.len() > max_len {
            let excess = self.samples.len() - max_len;
            self.samples.drain(..excess);
            self.start_frame += (excess / 2) as u64;
        }

        gap
    }

    fn start_frame(&self) -> u64 {
        self.start_frame
    }
}

struct VisualizerTap {
    audio_buffers: (BufferIndex, BufferIndex),
    buffer_count: usize,
    writer: AudioCaptureWriter,
    control: Arc<VisualizerControl>,
}

pub(crate) fn create_visualizer_tap(
    audio_buffers: (BufferIndex, BufferIndex),
    buffer_count: usize,
    writer: AudioCaptureWriter,
    control: Arc<VisualizerControl>,
) -> impl Tap + 'static {
    VisualizerTap {
        audio_buffers,
        buffer_count,
        writer,
        control,
    }
}

impl Tap for VisualizerTap {
    fn process(&mut self, context: &TapContext<'_>) {
        if !self.control.enabled() {
            return;
        }

        let selected_buffer = self
            .control
            .selected()
            .filter(|&index| index < self.buffer_count);

        match selected_buffer {
            Some(index) => self
                .writer
                .push_mono(context.read(BufferIndex::Internal(index))),
            None => self.writer.push_stereo(
                context.read(self.audio_buffers.0),
                context.read(self.audio_buffers.1),
            ),
        }
    }
}

pub(crate) struct VisualizerUpdate {
    pub(crate) new_frames: usize,
    pub(crate) gap: bool,
}

pub(crate) struct AudioVisualizer {
    capture: AudioCaptureReader,
    sample_rate: f64,
    analyzer: SpectrumAnalyzer,
    peak: f32,
    last_frame: u64,
}

impl AudioVisualizer {
    pub(crate) fn new(capture: AudioCaptureReader, sample_rate: u32) -> Self {
        Self {
            capture,
            sample_rate: f64::from(sample_rate),
            analyzer: SpectrumAnalyzer::new(FFT_SIZE),
            peak: 0.0,
            last_frame: 0,
        }
    }

    pub(crate) fn update(&mut self) -> VisualizerUpdate {
        let gap = self.capture.pull();

        let frames = self.capture.samples.len() / 2;
        let total_frame = self.capture.start_frame + frames as u64;
        let new_frames = if gap {
            frames
        } else {
            total_frame.saturating_sub(self.last_frame) as usize
        };
        self.last_frame = total_frame;

        let samples = &self.capture.samples;
        let start = frames.saturating_sub(PEAK_WINDOW_FRAMES) * 2;

        let mut peak = 0.0f32;
        for frame in samples[start..].as_chunks::<2>().0 {
            peak = peak.max(frame[0].abs()).max(frame[1].abs());
        }

        self.peak = if gap {
            peak
        } else {
            (self.peak * 0.9).max(peak)
        };

        VisualizerUpdate { new_frames, gap }
    }

    pub(crate) fn clear_history(&mut self) {
        self.capture.clear();
        self.last_frame = self.capture.start_frame();
        self.peak = 0.0;
    }

    pub(crate) fn samples(&self) -> &[f32] {
        &self.capture.samples
    }

    pub(crate) fn start_frame(&self) -> u64 {
        self.capture.start_frame()
    }

    pub(crate) fn gain(&self) -> f32 {
        if self.peak > 1e-4 {
            (0.9 / self.peak).clamp(0.1, 32.0)
        } else {
            1.0
        }
    }

    pub(crate) fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    pub(crate) fn window_frames(&self) -> usize {
        self.analyzer.size
    }

    pub(crate) fn spectrum(&mut self) -> &[f64] {
        self.analyzer.analyze(&self.capture.samples);
        self.analyzer.magnitudes()
    }
}

struct Complex {
    re: f64,
    im: f64,
}

struct SpectrumAnalyzer {
    size: usize,
    window: Vec<f64>,
    window_gain: f64,
    re: Vec<f64>,
    im: Vec<f64>,
    twiddles: Vec<Complex>,
    magnitudes: Vec<f64>,
    other_magnitudes: Vec<f64>,
}

impl SpectrumAnalyzer {
    fn new(size: usize) -> Self {
        assert!(size > 1 && size.is_power_of_two());

        let window = (0..size)
            .map(|i| 0.5 - 0.5 * (TAU * i as f64 / (size - 1) as f64).cos())
            .collect::<Vec<_>>();
        let window_gain = window.iter().sum();

        let twiddles = (0..size / 2)
            .map(|i| {
                let angle = -TAU * i as f64 / size as f64;
                Complex {
                    re: angle.cos(),
                    im: angle.sin(),
                }
            })
            .collect();

        Self {
            size,
            window,
            window_gain,
            re: vec![0.0; size],
            im: vec![0.0; size],
            twiddles,
            magnitudes: vec![MIN_DB; size / 2],
            other_magnitudes: vec![MIN_DB; size / 2],
        }
    }

    fn analyze(&mut self, stereo_samples: &[f32]) {
        self.analyze_channel(stereo_samples, 0);
        self.other_magnitudes.copy_from_slice(&self.magnitudes);
        self.analyze_channel(stereo_samples, 1);

        for (magnitude, other) in self.magnitudes.iter_mut().zip(&self.other_magnitudes) {
            *magnitude = magnitude.max(*other);
        }
    }

    fn analyze_channel(&mut self, stereo_samples: &[f32], channel: usize) {
        let size = self.size;
        let frames = stereo_samples.len() / 2;
        let used = frames.min(size);
        let start_frame = frames - used;

        self.re.fill(0.0);
        self.im.fill(0.0);

        for i in 0..used {
            let sample = f64::from(stereo_samples[2 * (start_frame + i) + channel]);
            self.re[i] = sample * self.window[i];
        }

        self.fft();

        let scale = 2.0 / self.window_gain;
        for (k, magnitude) in self.magnitudes.iter_mut().enumerate() {
            let power = self.re[k] * self.re[k] + self.im[k] * self.im[k];
            let db = 20.0 * (power.sqrt() * scale).max(1e-6).log10();
            *magnitude = db.max(MIN_DB);
        }
    }

    fn magnitudes(&self) -> &[f64] {
        &self.magnitudes
    }

    fn fft(&mut self) {
        let n = self.size;

        let mut j = 0;
        for i in 1..n {
            let mut bit = n >> 1;
            while j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j ^= bit;

            if i < j {
                self.re.swap(i, j);
                self.im.swap(i, j);
            }
        }

        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let step = n / len;

            for start in (0..n).step_by(len) {
                for k in 0..half {
                    let twiddle = &self.twiddles[k * step];
                    let a = start + k;
                    let b = a + half;

                    let re = twiddle.re * self.re[b] - twiddle.im * self.im[b];
                    let im = twiddle.re * self.im[b] + twiddle.im * self.re[b];

                    self.re[b] = self.re[a] - re;
                    self.im[b] = self.im[a] - im;
                    self.re[a] += re;
                    self.im[a] += im;
                }
            }

            len <<= 1;
        }
    }
}

// Correlation-based trigger using the middle of the previous frame as a template.
// Every sample in the correlation window has equal weight.
#[derive(Default)]
pub(crate) struct CorrelationTrigger {
    last_trigger: Option<i64>,
    template: Vec<f32>,
}

impl CorrelationTrigger {
    pub(crate) fn reset(&mut self) {
        self.last_trigger = None;
        self.template.clear();
    }

    pub(crate) fn next(
        &mut self,
        samples: &[f32],
        start_frame: u64,
        frames: usize,
        sweep: usize,
    ) -> i64 {
        debug_assert!(frames >= sweep);
        debug_assert!(sweep > 0);

        let start = start_frame as i64;
        let latest = start + frames as i64;
        let correlation_window = (sweep / CORRELATION_WINDOW_DIVISOR).max(1);
        let template_center = (sweep - correlation_window) / 2;
        let half_width = (sweep / SEARCH_RADIUS_DIVISOR).max(1) as i64;
        let sweep_i64 = sweep as i64;

        let Some(last_trigger) = self.last_trigger else {
            let trigger = latest - sweep_i64;
            self.adopt_template(samples, start, trigger, template_center, correlation_window);
            self.last_trigger = Some(trigger);
            return trigger;
        };

        let search_center = last_trigger + sweep_i64;

        // The previous frame is no longer in the capture buffer after a long stall.
        if search_center - half_width <= start {
            let trigger = latest - sweep_i64;
            self.adopt_template(samples, start, trigger, template_center, correlation_window);
            self.last_trigger = Some(trigger);
            return trigger;
        }

        // Wait until the rightmost candidate plus a complete frame is available.
        if search_center + half_width + sweep_i64 > latest {
            return last_trigger;
        }

        // Start with the expected offset. Equal correlations keep it, which avoids jumping
        // between equivalent matches.
        let center_local = (search_center - start) as usize + template_center;
        let mut best_offset = 0;
        let mut best_correlation = correlation(samples, center_local, &self.template);

        for offset in -half_width..=half_width {
            if offset == 0 {
                continue;
            }

            let trigger = search_center + offset;
            let local = (trigger - start) as usize + template_center;
            let correlation = correlation(samples, local, &self.template);

            if correlation > best_correlation {
                best_correlation = correlation;
                best_offset = offset;
            }
        }

        let trigger = search_center + best_offset;

        self.adopt_template(samples, start, trigger, template_center, correlation_window);
        self.last_trigger = Some(trigger);
        trigger
    }

    fn adopt_template(
        &mut self,
        samples: &[f32],
        start: i64,
        trigger: i64,
        template_center: usize,
        template_len: usize,
    ) {
        self.template.clear();
        self.template.reserve(template_len);

        let local = (trigger - start) as usize + template_center;
        for offset in 0..template_len {
            self.template.push(mono(samples, local + offset));
        }
    }
}

// Normalized cross-correlation over the template window, with equal weight for every sample.
pub(crate) fn correlation(samples: &[f32], start: usize, template: &[f32]) -> f64 {
    let length = template.len() as f64;
    let mut sum_x = 0.0;
    let mut sum_x_sq = 0.0;
    let mut sum_t = 0.0;
    let mut sum_t_sq = 0.0;
    let mut sum_xt = 0.0;

    for (offset, &template_sample) in template.iter().enumerate() {
        let template_sample = f64::from(template_sample);
        let sample = f64::from(mono(samples, start + offset));

        sum_x += sample;
        sum_x_sq += sample * sample;
        sum_t += template_sample;
        sum_t_sq += template_sample * template_sample;
        sum_xt += sample * template_sample;
    }

    let covariance = sum_xt - sum_x * sum_t / length;
    let variance_x = sum_x_sq - sum_x * sum_x / length;
    let variance_template = sum_t_sq - sum_t * sum_t / length;

    if variance_x <= 1e-12 || variance_template <= 1e-12 {
        0.0
    } else {
        (covariance / (variance_x.sqrt() * variance_template.sqrt())).clamp(-1.0, 1.0)
    }
}

fn mono(samples: &[f32], frame: usize) -> f32 {
    0.5 * (samples[2 * frame] + samples[2 * frame + 1])
}

pub(crate) fn level_color(level: f32) -> [u8; 4] {
    let level = level.clamp(0.0, 1.0).powf(0.6);
    let mut color = LEVEL_STOPS[0].1;

    for window in LEVEL_STOPS.windows(2) {
        let (start_level, start_color) = window[0];
        let (end_level, end_color) = window[1];

        if level <= end_level {
            let t = (level - start_level) / (end_level - start_level);
            color = [
                start_color[0] + (end_color[0] - start_color[0]) * t,
                start_color[1] + (end_color[1] - start_color[1]) * t,
                start_color[2] + (end_color[2] - start_color[2]) * t,
            ];
            break;
        }
    }

    [
        (color[0] * 255.0) as u8,
        (color[1] * 255.0) as u8,
        (color[2] * 255.0) as u8,
        255,
    ]
}
