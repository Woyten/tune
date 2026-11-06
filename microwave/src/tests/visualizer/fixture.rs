use crate::visualizer::AudioCaptureWriter;
use crate::visualizer::AudioVisualizer;
use crate::visualizer::CorrelationTrigger;
use crate::visualizer::VisualizerUpdate;
use crate::visualizer::create_audio_capture;

pub struct VisualizerFixture {
    writer: AudioCaptureWriter,
    visualizer: AudioVisualizer,
}

impl VisualizerFixture {
    pub fn new(sample_rate: u32) -> Self {
        let (writer, reader) = create_audio_capture();

        Self {
            writer,
            visualizer: AudioVisualizer::new(reader, sample_rate),
        }
    }

    pub fn push_stereo(&mut self, left: &[f64], right: &[f64]) {
        self.writer.push_stereo(left, right);
    }

    pub fn push_mono(&mut self, samples: &[f64]) {
        self.writer.push_mono(samples);
    }

    pub fn update(&mut self) -> VisualizerUpdate {
        self.visualizer.update()
    }

    pub fn clear_history(&mut self) {
        self.visualizer.clear_history();
    }

    pub fn samples(&self) -> &[f32] {
        self.visualizer.samples()
    }

    pub fn start_frame(&self) -> u64 {
        self.visualizer.start_frame()
    }

    pub fn window_frames(&self) -> usize {
        self.visualizer.window_frames()
    }

    pub fn spectrum(&mut self) -> &[f64] {
        self.visualizer.spectrum()
    }
}

pub struct TriggerFixture {
    pub visualizer: VisualizerFixture,
    trigger: CorrelationTrigger,
}

impl TriggerFixture {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            visualizer: VisualizerFixture::new(sample_rate),
            trigger: CorrelationTrigger::default(),
        }
    }

    pub fn push_mono(&mut self, samples: &[f64]) {
        self.visualizer.push_mono(samples);
    }

    pub fn next(&mut self) -> i64 {
        self.visualizer.update();

        let samples = self.visualizer.samples();
        let frames = samples.len() / 2;
        let start_frame = self.visualizer.start_frame();
        let window_frames = self.visualizer.window_frames();

        self.trigger
            .next(samples, start_frame, frames, window_frames)
    }
}

pub fn sine(sample_rate: f64, frequency: f64, frames: usize) -> Vec<f64> {
    (0..frames)
        .map(|frame| (std::f64::consts::TAU * frequency * frame as f64 / sample_rate).sin())
        .collect()
}
