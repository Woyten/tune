use std::{collections::HashMap, iter, sync::Arc};

use chrono::Local;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, Device, FromSample, Sample, SampleFormat, SampleRate, SizedSample, Stream,
    StreamConfig, SupportedBufferSize, SupportedStreamConfig,
};
use crossbeam::channel::{self, Receiver, Sender};
use hound::{WavSpec, WavWriter};
use log::{error, info, warn};
use magnetron::{
    automation::{AutomatableValue, Automated, AutomatedValue},
    buffer::BufferIndex,
    creator::Creator,
    stage::{Stage, StageActivity},
    Magnetron,
};
use ringbuf::{HeapRb, Producer};
use serde::{Deserialize, Serialize};

use crate::{
    control::{LiveParameter, LiveParameterStorage},
    portable::{self, WriteAndSeek},
    profile::{MainAutomatableValue, MainPipeline, Resources},
};

pub fn get_output_stream_params(
    output_buffer_size: u32,
    sample_rate_hz: Option<u32>,
) -> (Device, StreamConfig, SampleFormat) {
    let device = cpal::default_host().default_output_device().unwrap();
    let default_config = device.default_output_config().unwrap();
    let used_config = create_stream_config(
        "output",
        &default_config,
        output_buffer_size,
        sample_rate_hz.map(SampleRate),
    );

    info!("Using sample rate {} Hz", used_config.sample_rate.0);

    (device, used_config, default_config.sample_format())
}

pub fn start_context(
    output_stream_params: (Device, StreamConfig, SampleFormat),
    buffer_size: u32,
    num_buffers: usize,
    audio_buffers: (usize, usize),
    stages: MainPipeline,
    wav_file_prefix: String,
    storage: LiveParameterStorage,
    storage_updates: Receiver<LiveParameterStorage>,
    globals: Vec<(String, AutomatedValue<MainAutomatableValue>)>,
) -> Stream {
    let (recording_action_send, recording_action_recv) = channel::unbounded();

    let sample_rate_hz = output_stream_params.1.sample_rate.0;
    let context = AudioOutContext {
        magnetron: Magnetron::new(
            f64::from(sample_rate_hz).recip(),
            num_buffers,
            2 * usize::try_from(buffer_size).unwrap(),
        ), // The first invocation of cpal uses the double buffer size
        audio_buffers,
        stages,
        storage,
        storage_updates,
        globals_evaluated: globals
            .iter()
            .map(|(name, _)| (name.to_owned(), 0.0))
            .collect(),
        globals,
        wav_writer: None,
        sample_rate_hz,
        wav_file_prefix: wav_file_prefix.into(),
        recording_action_send,
        recording_action_recv,
    };

    context.start(output_stream_params)
}

struct AudioOutContext {
    magnetron: Magnetron,
    audio_buffers: (usize, usize),
    stages: MainPipeline,
    storage: LiveParameterStorage,
    storage_updates: Receiver<LiveParameterStorage>,
    globals: Vec<(String, AutomatedValue<MainAutomatableValue>)>,
    globals_evaluated: HashMap<String, f64>,
    wav_writer: Option<WavWriter<Box<dyn WriteAndSeek>>>,
    sample_rate_hz: u32,
    wav_file_prefix: Arc<str>,
    recording_action_send: Sender<RecordingAction>,
    recording_action_recv: Receiver<RecordingAction>,
}

impl AudioOutContext {
    fn start(
        self,
        (device, stream_config, sample_format): (Device, StreamConfig, SampleFormat),
    ) -> Stream {
        let stream = match sample_format {
            SampleFormat::F32 => self.create_stream::<f32>(&device, &stream_config),
            SampleFormat::I16 => self.create_stream::<i16>(&device, &stream_config),
            _ => panic!("Unsupported sample format {sample_format}"),
        };
        stream.play().unwrap();
        stream
    }

    fn create_stream<T: SizedSample + FromSample<f64>>(
        mut self,
        device: &Device,
        config: &StreamConfig,
    ) -> Stream
    where
        f32: FromSample<T>,
    {
        device
            .build_output_stream(
                config,
                move |buffer: &mut [T], _| {
                    self.render(buffer);
                },
                |err| error!("{err}"),
                None,
            )
            .unwrap()
    }

    fn render<T: Sample + FromSample<f64>>(&mut self, audio_buffer: &mut [T])
    where
        f32: FromSample<T>,
    {
        let foot_before = self.storage.is_active(LiveParameter::Foot);
        for storage_update in self.storage_updates.try_iter() {
            self.storage = storage_update;
        }
        let foot_after = self.storage.is_active(LiveParameter::Foot);
        if foot_after != foot_before {
            self.set_recording_active(foot_after)
        }

        let mut reset = false;
        for recording_action in self.recording_action_recv.try_iter() {
            match recording_action {
                RecordingAction::Started(wav_writer) => {
                    self.wav_writer = Some(wav_writer);
                    reset = true;
                }
                RecordingAction::Stopped => self.wav_writer = None,
            }
        }

        let mut buffers = self.magnetron.prepare(audio_buffer.len() / 2, reset);

        let render_window_secs = buffers.render_window_secs();
        for (name, global) in &mut self.globals {
            let use_context = global.use_context(
                render_window_secs,
                (&(), &self.storage, &self.globals_evaluated),
            );

            if let Some(global_evaluated) = self.globals_evaluated.get_mut(name) {
                *global_evaluated = use_context;
            }
        }

        buffers.process(
            (&(), &self.storage, &self.globals_evaluated),
            self.stages.iter_mut(),
        );
        for ((&magnetron_l, &magnetron_r), audio) in iter::zip(
            buffers.read(BufferIndex::Internal(self.audio_buffers.0)),
            buffers.read(BufferIndex::Internal(self.audio_buffers.1)),
        )
        .zip(audio_buffer.chunks_mut(2))
        {
            if let [audio_l, audio_r] = audio {
                *audio_l = T::from_sample(magnetron_l);
                *audio_r = T::from_sample(magnetron_r);
            }
        }
        if let Some(wav_writer) = &mut self.wav_writer {
            for &sample in &*audio_buffer {
                wav_writer.write_sample(f32::from_sample(sample)).unwrap();
            }
        }
    }

    fn set_recording_active(&self, recording_active: bool) {
        let recording_action = self.recording_action_send.clone();
        let sample_rate_hz = self.sample_rate_hz;
        let wav_file_prefix = self.wav_file_prefix.clone();
        portable::spawn_task(async move {
            let action = if recording_active {
                RecordingAction::Started(create_wav_writer(sample_rate_hz, &wav_file_prefix).await)
            } else {
                RecordingAction::Stopped
            };
            recording_action.send(action).unwrap();
        });
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioInSpec<A> {
    pub out_buffers: (usize, usize),
    pub out_levels: Option<(A, A)>,
}

impl<A: AutomatableValue> AudioInSpec<A> {
    pub fn create(
        &self,
        creator: &Creator<A>,
        buffer_size: u32,
        sample_rate: SampleRate,
        stages: &mut Vec<Stage<A>>,
        resources: &mut Resources,
    ) {
        const NUMBER_OF_CHANNELS: usize = 2;
        const EXCHANGE_BUFFER_BUFFER: usize = 4;
        let exchange_buffer_size =
            usize::try_from(buffer_size).unwrap() * EXCHANGE_BUFFER_BUFFER * NUMBER_OF_CHANNELS;

        let (audio_in_prod, mut audio_in_cons) = HeapRb::new(exchange_buffer_size).split();

        let context = AudioInContext {
            exchange_buffer: audio_in_prod,
        };

        match context.start(buffer_size, sample_rate) {
            None => {
                warn!("No default audio input device found");
                return;
            }
            Some(stream) => resources.push(Box::new(stream)),
        }

        let out_buffers = self.out_buffers;
        let buffer_size = usize::try_from(buffer_size).unwrap();
        let mut audio_in_synchronized = false;
        stages.push(
            creator.create_stage(&self.out_levels, move |buffers, out_levels| {
                if audio_in_cons.len() >= buffer_size {
                    if !audio_in_synchronized {
                        audio_in_synchronized = true;
                        info!("Audio-in synchronized");
                    }

                    buffers.read_0_write_2(
                        (
                            BufferIndex::Internal(out_buffers.0),
                            BufferIndex::Internal(out_buffers.1),
                        ),
                        out_levels,
                        || {
                            (
                                audio_in_cons.pop().unwrap_or_default(),
                                audio_in_cons.pop().unwrap_or_default(),
                            )
                        },
                    );
                } else if audio_in_synchronized {
                    audio_in_synchronized = false;
                    warn!("Audio-in desynchronized");
                }

                StageActivity::Internal
            }),
        )
    }
}

struct AudioInContext {
    exchange_buffer: Producer<f64, Arc<HeapRb<f64>>>,
}

impl AudioInContext {
    fn start(self, buffer_size: u32, sample_rate: SampleRate) -> Option<Stream> {
        let device = cpal::default_host().default_input_device()?;
        let default_config = device.default_input_config().unwrap();
        let used_config =
            create_stream_config("input", &default_config, buffer_size, Some(sample_rate));
        let sample_format = default_config.sample_format();
        let stream = match sample_format {
            SampleFormat::F32 => self.create_stream::<f32>(&device, &used_config),
            SampleFormat::I16 => self.create_stream::<i16>(&device, &used_config),
            _ => panic!("Unsupported sample format {sample_format}"),
        };
        stream.play().unwrap();
        Some(stream)
    }

    fn create_stream<T: SizedSample>(mut self, device: &Device, config: &StreamConfig) -> Stream
    where
        f64: FromSample<T>,
    {
        device
            .build_input_stream(
                config,
                move |buffer: &[T], _| {
                    self.exchange_buffer
                        .push_iter(&mut buffer[..].iter().map(|&s| f64::from_sample(s)));
                },
                |_| {},
                None,
            )
            .unwrap()
    }
}

fn create_stream_config(
    stream_type: &str,
    default_config: &SupportedStreamConfig,
    buffer_size: u32,
    sample_rate: Option<SampleRate>,
) -> StreamConfig {
    info!("Default {stream_type} stream config: {default_config:?}");
    let buffer_size = match default_config.buffer_size() {
        SupportedBufferSize::Range { .. } => BufferSize::Fixed(buffer_size),
        SupportedBufferSize::Unknown => {
            warn!("Cannot set buffer size on {stream_type} audio device. The device's default buffer size will be used.");
            BufferSize::Default
        }
    };

    StreamConfig {
        channels: 2,
        sample_rate: sample_rate.unwrap_or_else(|| default_config.sample_rate()),
        buffer_size,
    }
}

async fn create_wav_writer(
    sample_rate_hz: u32,
    file_prefix: &str,
) -> WavWriter<Box<dyn WriteAndSeek>> {
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

    info!("Created `{output_file_name}`");
    let write_and_seek: Box<dyn WriteAndSeek> =
        Box::new(portable::write_file(&output_file_name).await.unwrap());
    WavWriter::new(write_and_seek, spec).unwrap()
}

enum RecordingAction {
    Started(WavWriter<Box<dyn WriteAndSeek>>),
    Stopped,
}
