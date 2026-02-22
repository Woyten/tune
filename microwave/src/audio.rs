use std::iter;

use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;
use cpal::BufferSize;
use cpal::Device;
use cpal::FromSample;
use cpal::Sample;
use cpal::SampleFormat;
use cpal::SizedSample;
use cpal::Stream;
use cpal::StreamConfig;
use cpal::SupportedBufferSize;
use cpal::SupportedStreamConfig;
use magnetron::automation::AutomatableParam;
use magnetron::automation::Automated;
use magnetron::automation::AutomationFactory;
use magnetron::buffer::BufferIndex;
use magnetron::stage::Stage;
use magnetron::stage::StageActivity;
use ringbuf::traits::Consumer;
use ringbuf::traits::Observer;
use ringbuf::traits::Producer;
use ringbuf::traits::Split;
use ringbuf::HeapProd;
use ringbuf::HeapRb;
use serde::Deserialize;
use serde::Serialize;

use crate::pipeline::AudioPipeline;
use crate::Resources;

pub struct StreamParams {
    device: Device,
    config: StreamConfig,
    sample_format: SampleFormat,
    pub buffer_size: u32,
    pub sample_rate: u32,
}

pub fn get_output_stream_params(buffer_size: u32, sample_rate_hz: Option<u32>) -> StreamParams {
    let device = cpal::default_host().default_output_device().unwrap();
    let default_config = device.default_output_config().unwrap();
    let used_config = create_stream_config("output", &default_config, buffer_size, sample_rate_hz);

    log::info!("Using sample rate {} Hz", used_config.sample_rate);

    StreamParams {
        device,
        sample_format: default_config.sample_format(),
        buffer_size,
        sample_rate: used_config.sample_rate,
        config: used_config,
    }
}

pub fn start_context(
    resources: &mut Resources,
    stream_params: &StreamParams,
    pipeline: AudioPipeline,
) {
    let context = AudioOutContext { pipeline };

    resources.push(Box::new(context.start(stream_params)));
}

struct AudioOutContext {
    pipeline: AudioPipeline,
}

impl AudioOutContext {
    fn start(self, stream_params: &StreamParams) -> Stream {
        let stream = match stream_params.sample_format {
            SampleFormat::F32 => self.create_stream::<f32>(stream_params),
            SampleFormat::I16 => self.create_stream::<i16>(stream_params),
            other => panic!("Unsupported sample format {other}"),
        };
        stream.play().unwrap();
        stream
    }

    fn create_stream<T: SizedSample + FromSample<f64>>(
        mut self,
        stream_params: &StreamParams,
    ) -> Stream
    where
        f32: FromSample<T>,
    {
        stream_params
            .device
            .build_output_stream(
                &stream_params.config,
                move |buffer: &mut [T], _| {
                    self.render(buffer);
                },
                |err| log::error!("Error in main audio thread: {err}"),
                None,
            )
            .unwrap()
    }

    fn render<T: Sample + FromSample<f64>>(&mut self, audio_buffer: &mut [T])
    where
        f32: FromSample<T>,
    {
        let audio_buffers = self.pipeline.audio_buffers();

        let buffers = self.pipeline.render(audio_buffer.len() / 2);

        for ((&magnetron_l, &magnetron_r), audio) in iter::zip(
            buffers.read(BufferIndex::Internal(audio_buffers.0)),
            buffers.read(BufferIndex::Internal(audio_buffers.1)),
        )
        .zip(audio_buffer.as_chunks_mut().0)
        {
            *audio = [magnetron_l, magnetron_r].map(T::from_sample);
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct AudioInSpec<A> {
    pub out_buffers: (usize, usize),
    pub out_levels: Option<(A, A)>,
}

impl<A: AutomatableParam> AudioInSpec<A> {
    pub fn create(
        &self,
        resources: &mut Resources,
        buffer_size: u32,
        sample_rate: u32,
        factory: &mut AutomationFactory<A>,
        stages: &mut Vec<Stage<A>>,
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
                log::warn!("No default audio input device found");
                return;
            }
            Some(stream) => resources.push(Box::new(stream)),
        }

        let out_buffers = self.out_buffers;
        let buffer_size = usize::try_from(buffer_size).unwrap();
        let mut audio_in_synchronized = false;
        stages.push(
            factory
                .automate(&self.out_levels)
                .into_stage(move |buffers, out_levels| {
                    if audio_in_cons.occupied_len() >= buffer_size {
                        if !audio_in_synchronized {
                            audio_in_synchronized = true;
                            log::info!("Audio-in synchronized");
                        }

                        buffers.read_0_write_2(
                            (
                                BufferIndex::Internal(out_buffers.0),
                                BufferIndex::Internal(out_buffers.1),
                            ),
                            out_levels,
                            || {
                                (
                                    audio_in_cons.try_pop().unwrap_or_default(),
                                    audio_in_cons.try_pop().unwrap_or_default(),
                                )
                            },
                        );
                    } else if audio_in_synchronized {
                        audio_in_synchronized = false;
                        log::warn!("Audio-in desynchronized");
                    }

                    StageActivity::Internal
                }),
        )
    }
}

struct AudioInContext {
    exchange_buffer: HeapProd<f64>,
}

impl AudioInContext {
    fn start(self, buffer_size: u32, sample_rate: u32) -> Option<Stream> {
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
                |err| log::error!("Error in audio recording thread: {err}"),
                None,
            )
            .unwrap()
    }
}

fn create_stream_config(
    stream_type: &str,
    default_config: &SupportedStreamConfig,
    buffer_size: u32,
    sample_rate: Option<u32>,
) -> StreamConfig {
    log::info!("Default {stream_type} stream config: {default_config:?}");
    let buffer_size = match default_config.buffer_size() {
        SupportedBufferSize::Range { .. } => BufferSize::Fixed(buffer_size),
        SupportedBufferSize::Unknown => {
            log::warn!("Cannot set buffer size on {stream_type} audio device. The device's default buffer size will be used.");
            BufferSize::Default
        }
    };

    StreamConfig {
        channels: 2,
        sample_rate: sample_rate.unwrap_or_else(|| default_config.sample_rate()),
        buffer_size,
    }
}
