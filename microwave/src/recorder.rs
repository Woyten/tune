use std::sync::Arc;
use std::time::Duration;

use async_ringbuf::traits::AsyncConsumer;
use async_ringbuf::AsyncHeapCons;
use async_ringbuf::AsyncHeapRb;
use async_std::task;
use chrono::Local;
use flume::Sender;
use hound::WavSpec;
use hound::WavWriter;
use magnetron::automation::AutomatableParam;
use magnetron::automation::Automated;
use magnetron::automation::AutomationFactory;
use magnetron::buffer::BufferIndex;
use magnetron::stage::Stage;
use magnetron::stage::StageActivity;
use ringbuf::traits::Consumer;
use ringbuf::traits::Producer;
use ringbuf::traits::RingBuffer;
use ringbuf::traits::Split;
use ringbuf::LocalRb;
use serde::Deserialize;
use serde::Serialize;

use crate::portable;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WavRecorderSpec<A> {
    pub in_buffers: (usize, usize),
    pub num_back_samples: usize,
    pub file_prefix: String,
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
        //const RECORDING_HEADROOM: usize = 4;

        let (mut recording_prod, recording_cons) = AsyncHeapRb::new(
            //self.num_back_samples + usize::try_from(buffer_size).unwrap() * RECORDING_HEADROOM,
            self.num_back_samples,
        )
        .split();

        let in_buffers = self.in_buffers;
        let num_back_samples = self.num_back_samples;
        let file_prefix = self.file_prefix.clone().into();
        let stage_index = stages.len();
        let events = events.clone();

        portable::spawn_task(async move {
            WavRecorder {
                sample_rate,
                in_buffers,
                num_back_samples,
                file_prefix,
                stage_index,
                recording_buffer: recording_cons,
                events,
            }
            .start_loop()
            .await
        });

        let stage = factory.automate(&self.recording_active).into_stage(
            move |buffers, recording_active| {
                if buffers.reset() {
                    let _ = recording_prod.try_push(WavRecorderStageEvent::Reset);
                }

                let _ = recording_prod
                    .try_push(WavRecorderStageEvent::SetActive(recording_active >= 0.5));

                recording_prod.push_iter(
                    buffers
                        .read(BufferIndex::Internal(in_buffers.0))
                        .iter()
                        .zip(buffers.read(BufferIndex::Internal(in_buffers.1)))
                        .map(|(&left, &right)| WavRecorderStageEvent::Sample(left, right)),
                );

                StageActivity::Observer
            },
        );

        stages.push(stage);
    }
}

struct WavRecorder<E> {
    sample_rate: u32,
    in_buffers: (usize, usize),
    num_back_samples: usize,
    file_prefix: Arc<str>,
    stage_index: usize,
    recording_buffer: AsyncHeapCons<WavRecorderStageEvent>,
    events: Sender<E>,
}

enum WavRecorderStageEvent {
    SetActive(bool),
    Sample(f64, f64),
    Reset,
}

impl<E: From<WavRecorderEvent>> WavRecorder<E> {
    async fn start_loop(&mut self) {
        let mut back_samples = LocalRb::new(self.num_back_samples);
        let mut maybe_wav_writer: Option<WavWriter<_>> = None;

        loop {
            if let Some(wav_writer) = &mut maybe_wav_writer {
                for (left, right) in back_samples.pop_iter() {
                    wav_writer.write_sample(left as f32).unwrap();
                    wav_writer.write_sample(right as f32).unwrap();
                }
            }

            if let Some(buffer_element) = self.recording_buffer.pop().await {
                match buffer_element {
                    WavRecorderStageEvent::SetActive(true) => {
                        if maybe_wav_writer.is_none() {
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

                            maybe_wav_writer = Some(
                                WavWriter::new(
                                    portable::write_file(&output_file_name).await.unwrap(),
                                    spec,
                                )
                                .unwrap(),
                            );

                            self.events
                                .send(
                                    WavRecorderEvent {
                                        in_buffers: self.in_buffers,
                                        file_name: Some(output_file_name),
                                        stage_index: self.stage_index,
                                    }
                                    .into(),
                                )
                                .unwrap();
                        }
                    }
                    WavRecorderStageEvent::SetActive(false) => {
                        if maybe_wav_writer.is_some() {
                            maybe_wav_writer = None;

                            self.events
                                .send(
                                    WavRecorderEvent {
                                        in_buffers: self.in_buffers,
                                        file_name: None,
                                        stage_index: self.stage_index,
                                    }
                                    .into(),
                                )
                                .unwrap();

                            task::sleep(Duration::from_secs(2)).await;
                        }
                    }
                    WavRecorderStageEvent::Sample(left, right) => {
                        back_samples.push_overwrite((left, right));
                    }
                    WavRecorderStageEvent::Reset => {
                        back_samples.clear();

                        if maybe_wav_writer.is_some() {
                            maybe_wav_writer = None;

                            self.events
                                .send(
                                    WavRecorderEvent {
                                        in_buffers: self.in_buffers,
                                        file_name: None,
                                        stage_index: self.stage_index,
                                    }
                                    .into(),
                                )
                                .unwrap();

                            task::sleep(Duration::from_secs(2)).await;
                        }
                    }
                }
            }
        }
    }
}

pub struct WavRecorderEvent {
    /// Used for retaining order of recorder stages.
    pub stage_index: usize,
    pub in_buffers: (usize, usize),
    pub file_name: Option<String>,
}
