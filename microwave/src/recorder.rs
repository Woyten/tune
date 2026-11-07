use std::io::BufWriter;

use chrono::Local;
use flume::Sender;
use hound::WavSpec;
use hound::WavWriter;
use magnetron::automation::Automated;
use magnetron::stage::Stage;
use magnetron::stage::StageActivity;
use ringbuf::HeapCons;
use ringbuf::HeapRb;
use ringbuf::traits::Consumer;
use ringbuf::traits::Observer;
use ringbuf::traits::RingBuffer;
use ringbuf::traits::Split;
use serde::Deserialize;
use serde::Serialize;

use crate::magnetron::BufferName;
use crate::magnetron::Factory;
use crate::pipeline::PipelineEvent;
use crate::portable;
use crate::portable::FileWrite;
use crate::profile::PipelineParam;
use crate::tap::AudioRingMessage;
use crate::tap::AudioRingProducer;

const RECORDING_SECONDS: usize = 10;
#[cfg(not(target_arch = "wasm32"))]
const WORKER_POLL_INTERVAL_MS: u64 = 5;
const MAX_WRITE_FRAMES_PER_POLL: usize = 4096;

#[derive(Deserialize, Serialize)]
pub struct WavRecorderSpec<A> {
    pub in_buffers: (BufferName, BufferName),
    pub skip_back_secs: f64,
    pub file_prefix: String,
    pub recording_active: A,
}

impl WavRecorderSpec<PipelineParam> {
    pub(crate) fn create(
        &self,
        factory: &mut Factory<PipelineParam>,
        sample_rate_hz: u32,
        stages: &mut Vec<Stage<PipelineParam>>,
        workers: &mut Vec<WavRecorderWorker>,
        events: &Sender<PipelineEvent>,
    ) {
        let in_buffers = factory.internal_buffer_indices((&self.in_buffers.0, &self.in_buffers.1));
        let recording_active = factory.automate(&self.recording_active);
        let skip_back_frames = (self.skip_back_secs * f64::from(sample_rate_hz)).round() as usize;
        let has_skip_back = skip_back_frames > 0;
        let capacity = usize::try_from(sample_rate_hz).unwrap() * RECORDING_SECONDS;
        let (producer, consumer) = HeapRb::<AudioRingMessage>::new(capacity + 1).split();
        let index = stages.len();

        let stage = recording_active.into_stage({
            let mut input = RecorderInput::new(AudioRingProducer::new(producer), has_skip_back);

            move |buffers, recording_active| {
                input.process(
                    recording_active >= 0.5,
                    buffers.read(in_buffers.0),
                    buffers.read(in_buffers.1),
                );
                StageActivity::Observer
            }
        });
        stages.push(stage);

        workers.push(WavRecorderWorker {
            index,
            in_buffer_names: self.in_buffers.clone(),
            file_prefix: self.file_prefix.clone(),
            sample_rate_hz,
            consumer,
            skip_back_history: has_skip_back.then(|| HeapRb::new(skip_back_frames)),
            skip_back_flush: false,
            recording: false,
            events: events.clone(),
            writer: None,
            failed: false,
            file_index: 0,
        });
    }
}

// Control messages and audio frames share one SPSC ring, so the worker sees recording
// start/stop and overflow in the same order as the audio.
struct RecorderInput {
    ring: AudioRingProducer,
    skip_back: bool,
    was_active: bool,
    pending_start: bool,
    pending_end: bool,
    overflowed: bool,
}

impl RecorderInput {
    fn new(ring: AudioRingProducer, skip_back: bool) -> Self {
        Self {
            ring,
            skip_back,
            was_active: false,
            pending_start: false,
            pending_end: false,
            overflowed: false,
        }
    }

    fn process(&mut self, active: bool, left: &[f64], right: &[f64]) {
        if active != self.was_active {
            if active {
                self.pending_start = true;
                self.overflowed = false;
            } else {
                self.pending_start = false;
                self.pending_end = true;
            }
            self.was_active = active;
        }

        if !self.flush() {
            return;
        }

        if self.overflowed || self.pending_start || self.pending_end {
            return;
        }

        if !active && !self.skip_back {
            return;
        }

        for (&left, &right) in left.iter().zip(right) {
            if !self.ring.push_frame(left as f32, right as f32) {
                if active {
                    self.overflowed = true;
                }
                break;
            }
        }
    }

    // Try to enqueue pending control messages. Keep them queued locally until there is room.
    fn flush(&mut self) -> bool {
        if !self.ring.flush_overflow() {
            return false;
        }

        if self.pending_end {
            if !self.ring.try_push_control(AudioRingMessage::End) {
                return false;
            }
            self.pending_end = false;
        }

        if self.pending_start {
            if !self.ring.try_push_control(AudioRingMessage::Start) {
                return false;
            }
            self.pending_start = false;
        }

        true
    }
}

pub(crate) struct WavRecorderWorker {
    index: usize,
    in_buffer_names: (BufferName, BufferName),
    file_prefix: String,
    sample_rate_hz: u32,
    consumer: HeapCons<AudioRingMessage>,
    // Collected only while idle. Starting a recording flushes it into the file;
    // ending a recording clears it, so the next take starts cold.
    skip_back_history: Option<HeapRb<(f32, f32)>>,
    skip_back_flush: bool,
    recording: bool,
    events: Sender<PipelineEvent>,
    writer: Option<WavWriter<BufWriter<FileWrite>>>,
    failed: bool,
    file_index: u32,
}

impl WavRecorderWorker {
    fn poll(&mut self) {
        for _ in 0..MAX_WRITE_FRAMES_PER_POLL {
            if self.skip_back_flush && self.flush_skip_back_chunk() {
                return;
            }

            let Some(message) = self.consumer.try_pop() else {
                break;
            };

            match message {
                AudioRingMessage::Start => {
                    if !self.finalize_writer() {
                        continue;
                    }

                    self.failed = false;
                    self.recording = true;
                    if let Err(error) = self.start_recording() {
                        log::error!("Could not create WAV recorder file: {error}");
                        self.fail("Recording stopped: could not create the WAV file");
                    }
                }
                AudioRingMessage::Frame { left, right } => {
                    if self.recording {
                        if self.failed {
                            continue;
                        }

                        let Some(writer) = self.writer.as_mut() else {
                            continue;
                        };

                        if let Err(error) = write_frame(writer, left, right) {
                            log::error!("Could not write WAV recorder file: {error}");
                            self.writer = None;
                            self.fail("Recording stopped: could not write the WAV file");
                        }
                    } else if let Some(history) = &mut self.skip_back_history {
                        history.push_overwrite((left, right));
                    }
                }
                AudioRingMessage::End => {
                    self.recording = false;
                    self.skip_back_flush = false;
                    if !self.failed && self.finalize_writer() {
                        self.send_event(None, None);
                    }
                    if let Some(history) = &mut self.skip_back_history {
                        history.clear();
                    }
                }
                AudioRingMessage::Overflow => {
                    self.skip_back_flush = false;
                    if self.recording {
                        self.finalize_writer();
                        self.fail(
                            "Recording stopped: the recording buffer overflowed and samples were dropped",
                        );
                    } else {
                        log::warn!(
                            "WAV recorder {} skip-back history overflowed; history cleared",
                            self.file_prefix
                        );
                        if let Some(history) = &mut self.skip_back_history {
                            history.clear();
                        }
                    }
                }
            }
        }
    }

    fn flush_skip_back_chunk(&mut self) -> bool {
        // Keep the wasm update loop responsive. Half a second of audio per poll drains
        // a 60 s history in about 120 polls, well before the 10 s ring fills.
        let budget = (usize::try_from(self.sample_rate_hz).unwrap() / 2).max(1);
        let mut written = 0;
        let mut has_more = false;
        let mut error = None;

        if let (Some(writer), Some(history)) =
            (self.writer.as_mut(), self.skip_back_history.as_mut())
        {
            for (left, right) in history.pop_iter().take(budget) {
                if let Err(err) = write_frame(writer, left, right) {
                    error = Some(err);
                    break;
                }
                written += 1;
            }
            has_more = written == budget && !history.is_empty();
        }

        if let Some(error) = error {
            log::error!("Could not write WAV recorder file: {error}");
            self.writer = None;
            self.fail("Recording stopped: could not write the WAV file");
            self.skip_back_flush = false;
            if let Some(history) = &mut self.skip_back_history {
                history.clear();
            }
            return false;
        }

        if !has_more {
            self.skip_back_flush = false;
        }
        has_more
    }

    fn start_recording(&mut self) -> Result<(), String> {
        self.file_index += 1;
        let (writer, file_name) =
            create_wav_writer(self.sample_rate_hz, &self.file_prefix, self.file_index)?;

        self.writer = Some(writer);
        self.skip_back_flush = self.skip_back_history.is_some();
        self.send_event(Some(file_name), None);
        Ok(())
    }

    fn finalize_writer(&mut self) -> bool {
        let Some(writer) = self.writer.take() else {
            return true;
        };

        if let Err(error) = writer.finalize() {
            log::error!("Could not finalize WAV recorder file: {error}");
            self.fail("Recording stopped: could not finalize the WAV file");
            return false;
        }

        true
    }

    fn fail(&mut self, message: &'static str) {
        if self.failed {
            return;
        }

        self.failed = true;
        log::error!("WAV recorder '{}': {message}", self.file_prefix);
        self.send_event(None, Some(message.to_owned()));
    }

    fn send_event(&self, file_name: Option<String>, error: Option<String>) {
        let _ = self.events.send(
            WavRecorderEvent {
                index: self.index,
                in_buffers: self.in_buffer_names.clone(),
                file_name,
                error,
            }
            .into(),
        );
    }
}

pub(crate) struct RecorderWorkers {
    workers: Vec<WavRecorderWorker>,
}

impl RecorderWorkers {
    pub(crate) fn new(workers: Vec<WavRecorderWorker>) -> Self {
        Self { workers }
    }

    pub(crate) fn start(self) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut workers = self.workers;
            for worker in workers.drain(..) {
                std::thread::Builder::new()
                    .name(format!("wav-recorder-{}", worker.index))
                    .spawn(move || {
                        let mut worker = worker;
                        loop {
                            worker.poll();
                            std::thread::sleep(std::time::Duration::from_millis(
                                WORKER_POLL_INTERVAL_MS,
                            ));
                        }
                    })
                    .expect("Could not spawn WAV recorder thread");
            }
            Self { workers }
        }

        // On wasm there are no background threads. The recorder is polled from the Bevy update
        // loop instead, so a throttled tab can drop samples from the bounded ring buffer.
        #[cfg(target_arch = "wasm32")]
        {
            self
        }
    }

    pub(crate) fn poll(&mut self) {
        for worker in &mut self.workers {
            worker.poll();
        }
    }
}

fn write_frame(
    writer: &mut WavWriter<BufWriter<FileWrite>>,
    left: f32,
    right: f32,
) -> Result<(), hound::Error> {
    writer.write_sample(left)?;
    writer.write_sample(right)
}

fn create_wav_writer(
    sample_rate_hz: u32,
    file_prefix: &str,
    file_index: u32,
) -> Result<(WavWriter<BufWriter<FileWrite>>, String), String> {
    let output_file_name = format!(
        "{}_{}_s{}.wav",
        file_prefix,
        Local::now().format("%Y%m%d_%H%M%S_%3f"),
        file_index
    );

    let spec = WavSpec {
        channels: 2,
        sample_rate: sample_rate_hz,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let write_and_seek = BufWriter::new(portable::create_file(&output_file_name)?);
    let writer = WavWriter::new(write_and_seek, spec).map_err(|error| error.to_string())?;

    Ok((writer, output_file_name))
}

#[derive(Debug, PartialEq, Eq)]
pub struct WavRecorderEvent {
    /// Stable identifier of the recorder configuration.
    pub index: usize,
    pub in_buffers: (BufferName, BufferName),
    pub file_name: Option<String>,
    pub error: Option<String>,
}
