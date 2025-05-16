use std::sync::Arc;

use chrono::Local;
use flume::Sender;
use hound::{WavSpec, WavWriter};
use magnetron::{
    automation::{AutomatableParam, Automated, AutomationFactory},
    buffer::BufferIndex,
    stage::{Stage, StageActivity},
};
use serde::{Deserialize, Serialize};

use crate::portable::{self, WriteAndSeek};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WavRecorderSpec<A> {
    pub in_buffers: (usize, usize),
    pub file_prefix: String,
    pub recording_active: A,
}

impl<A: AutomatableParam> WavRecorderSpec<A> {
    pub fn create<E: From<WavRecorderEvent> + Send + 'static>(
        &self,
        factory: &mut AutomationFactory<A>,
        stages: &mut Vec<Stage<A>>,
        events: &Sender<E>,
    ) {
        let in_buffers = self.in_buffers;
        let file_prefix = <Arc<str>>::from(self.file_prefix.clone());
        let index = stages.len();
        let events = events.clone();

        let (wav_writer_send, wav_writer_recv) = flume::unbounded();
        let mut state = RecorderState::None;

        let stage = factory.automate(&self.recording_active).into_stage(
            move |buffers, recording_active| {
                for (wav_writer, file_name) in wav_writer_recv.try_iter() {
                    state = RecorderState::Created(wav_writer);
                    events
                        .send(
                            WavRecorderEvent {
                                index,
                                in_buffers,
                                file_name: Some(file_name),
                            }
                            .into(),
                        )
                        .unwrap();
                }

                match state {
                    RecorderState::None => {
                        if recording_active >= 0.5 {
                            portable::spawn_task({
                                let wav_writer_send = wav_writer_send.clone();
                                let sample_rate_hz = buffers.sample_width_secs().recip() as u32;
                                let file_prefix = file_prefix.clone();

                                async move {
                                    wav_writer_send
                                        .send(create_wav_writer(sample_rate_hz, &file_prefix).await)
                                        .unwrap();
                                }
                            });

                            state = RecorderState::Creating;
                        }
                    }
                    RecorderState::Creating => {}
                    RecorderState::Created(ref mut wav_writer) => {
                        if recording_active >= 0.5 {
                            let left = buffers.read(BufferIndex::Internal(in_buffers.0));
                            let right = buffers.read(BufferIndex::Internal(in_buffers.1));
                            for (&l, &r) in left.iter().zip(right) {
                                wav_writer.write_sample(l as f32).unwrap();
                                wav_writer.write_sample(r as f32).unwrap();
                            }
                        } else {
                            state = RecorderState::None;
                            events
                                .send(
                                    WavRecorderEvent {
                                        index,
                                        in_buffers,
                                        file_name: None,
                                    }
                                    .into(),
                                )
                                .unwrap();
                        }
                    }
                }

                StageActivity::Observer
            },
        );

        stages.push(stage);
    }
}

enum RecorderState {
    None,
    Creating,
    Created(WavWriter<Box<dyn WriteAndSeek>>),
}

async fn create_wav_writer(
    sample_rate_hz: u32,
    file_prefix: &str,
) -> (WavWriter<Box<dyn WriteAndSeek>>, String) {
    let output_file_name = format!(
        "{}_{}.wav",
        file_prefix,
        Local::now().format("%Y%m%d_%H%M%S")
    );

    let spec = WavSpec {
        channels: 2,
        sample_rate: sample_rate_hz,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let write_and_seek: Box<dyn WriteAndSeek> =
        Box::new(portable::write_file(&output_file_name).await.unwrap());

    (
        WavWriter::new(write_and_seek, spec).unwrap(),
        output_file_name,
    )
}

pub struct WavRecorderEvent {
    /// Used for retaining order of recorder stages.
    pub index: usize,
    pub in_buffers: (usize, usize),
    pub file_name: Option<String>,
}
