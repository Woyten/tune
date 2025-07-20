use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use async_std::task;
use chrono::Local;
use flume::Sender;
use hound::{WavSpec, WavWriter};
use magnetron::{
    automation::{AutomatableParam, Automated, AutomationFactory},
    buffer::BufferIndex,
    stage::{Stage, StageActivity},
};
use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapCons, HeapRb,
};
use serde::{Deserialize, Serialize};

use crate::portable::{self, FileWrite};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WavRecorderSpec<A> {
    pub in_buffers: (usize, usize),
    pub file_prefix: String,
    pub num_back_samples: usize,
    pub recording_active: A,
}

impl<A: AutomatableParam> WavRecorderSpec<A> {
    pub fn create<E: From<WavRecorderEvent> + Send + 'static>(
        &self,
        sample_rate: u32,
        factory: &mut AutomationFactory<A>,
        stages: &mut Vec<Stage<A>>,
        events: &Sender<E>,
    ) {
        let in_buffers = self.in_buffers;
        let file_prefix = self.file_prefix.clone().into();
        let index = stages.len();
        let events = events.clone();

        let (recording_prod, mut recording_cons) = HeapRb::new(self.num_back_samples).split();

        portable::spawn_task(async move {
            WavRecorder {
                file_prefix,
                sample_rate,
                recording_buffer: recording_cons,
            }
            .start_loop()
            .await
        });

        let recording_active = false;

        let stage = factory.automate(&self.recording_active).into_stage(
            move |buffers, recording_active| {
                match recording_active {}

                StageActivity::Observer
            },
        );

        stages.push(stage);
    }
}

struct WavRecorder {
    file_prefix: Arc<str>,
    sample_rate: u32,
    // None means stop recording / don't record.
    recording_buffer: HeapCons<Option<(f64, f64)>>,
}

impl WavRecorder {
    async fn start_loop(&mut self) {
        let mut maybe_wav_writer = None;

        // TODO: Blocking used in async context
        for buffer_element in self.recording_buffer.pop_iter() {
            match buffer_element {
                Some((left, right)) => {
                    let mut wav_writer = match maybe_wav_writer.take() {
                        Some(wav_writer) => wav_writer,
                        None => {
                            let output_file_name = format!(
                                "{}_{}.wav",
                                self.file_prefix,
                                Local::now().format("%Y%m%d_%H%M%S")
                            );

                            let spec = WavSpec {
                                channels: 2,
                                sample_rate: self.sample_rate,
                                bits_per_sample: 32,
                                sample_format: hound::SampleFormat::Float,
                            };

                            WavWriter::new(
                                portable::write_file(&output_file_name).await.unwrap(),
                                spec,
                            )
                            .unwrap()

                            // TODO: Report event
                        }
                    };

                    wav_writer.write_sample(left as f32).unwrap();
                    wav_writer.write_sample(right as f32).unwrap();

                    maybe_wav_writer = Some(wav_writer)
                }
                None => {
                    if maybe_wav_writer.take().is_some() {
                        task::sleep(Duration::from_secs(2)).await;

                        // TODO: Report event
                    }
                }
            }
        }
    }
}

pub struct WavRecorderEvent {
    /// Used for retaining order of recorder stages.
    pub index: usize,
    pub in_buffers: (usize, usize),
    pub file_name: Option<String>,
}
