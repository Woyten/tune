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
const WORKER_POLL_INTERVAL_MS: u64 = 5;
const MAX_WRITE_FRAMES_PER_POLL: usize = 4096;

#[derive(Deserialize, Serialize)]
pub struct WavRecorderSpec<A> {
    pub in_buffers: (BufferName, BufferName),
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
        let capacity = usize::try_from(sample_rate_hz).unwrap() * RECORDING_SECONDS;
        let (producer, consumer) = HeapRb::<AudioRingMessage>::new(capacity + 1).split();
        let index = stages.len();

        let stage = recording_active.into_stage({
            let mut input = RecorderInput::new(AudioRingProducer::new(producer));

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
    was_active: bool,
    pending_start: bool,
    pending_end: bool,
    overflowed: bool,
}

impl RecorderInput {
    fn new(ring: AudioRingProducer) -> Self {
        Self {
            ring,
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

        if !active || self.overflowed || self.pending_start || self.pending_end {
            return;
        }

        for (&left, &right) in left.iter().zip(right) {
            if !self.ring.push_frame(left as f32, right as f32) {
                self.overflowed = true;
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
    events: Sender<PipelineEvent>,
    writer: Option<WavWriter<BufWriter<FileWrite>>>,
    failed: bool,
    file_index: u32,
}

impl WavRecorderWorker {
    fn poll(&mut self) {
        for _ in 0..MAX_WRITE_FRAMES_PER_POLL {
            let Some(message) = self.consumer.try_pop() else {
                break;
            };

            match message {
                AudioRingMessage::Start => {
                    if !self.finalize_writer() {
                        continue;
                    }

                    self.failed = false;
                    if let Err(error) = self.create_writer() {
                        log::error!("Could not create WAV recorder file: {error}");
                        self.fail("Recording stopped: could not create the WAV file");
                    }
                }
                AudioRingMessage::Frame { left, right } => {
                    if self.failed {
                        continue;
                    }

                    let Some(writer) = self.writer.as_mut() else {
                        continue;
                    };

                    let result = writer
                        .write_sample(left)
                        .and_then(|()| writer.write_sample(right));
                    if let Err(error) = result {
                        log::error!("Could not write WAV recorder file: {error}");
                        self.writer = None;
                        self.fail("Recording stopped: could not write the WAV file");
                    }
                }
                AudioRingMessage::End => {
                    if !self.failed && self.finalize_writer() {
                        self.send_event(None, None);
                    }
                }
                AudioRingMessage::Overflow => {
                    self.finalize_writer();
                    self.fail(
                        "Recording stopped: the recording buffer overflowed and samples were dropped",
                    );
                }
            }
        }
    }

    fn create_writer(&mut self) -> Result<(), String> {
        self.file_index += 1;
        let (writer, file_name) =
            create_wav_writer(self.sample_rate_hz, &self.file_prefix, self.file_index)?;
        self.writer = Some(writer);
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
