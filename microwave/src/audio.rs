use std::{fs::File, io::BufWriter, sync::Arc, thread};

use chrono::Local;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, Device, FromSample, Sample, SampleFormat, SampleRate, SizedSample, Stream,
    StreamConfig, SupportedBufferSize, SupportedStreamConfig,
};
use crossbeam::channel::{self, Receiver, Sender};
use hound::{WavSpec, WavWriter};
use magnetron::automation::AutomationContext;
use ringbuf::{HeapRb, Producer};

use crate::control::{LiveParameter, LiveParameterStorage};

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

    println!("[INFO] Using sample rate {} Hz", used_config.sample_rate.0);

    (device, used_config, default_config.sample_format())
}

pub struct AudioOptions {
    pub audio_in_enabled: bool,
    pub output_buffer_size: u32,
    pub input_buffer_size: u32,
    pub exchange_buffer_size: usize,
    pub wav_file_prefix: String,
}

pub struct AudioModel {
    // Not dead, actually. Audio-out is active as long as this Stream is not dropped.
    #[allow(dead_code)]
    output_stream: Stream,
    // Not dead, actually. Audio-in is active as long as this Stream is not dropped.
    #[allow(dead_code)]
    input_stream: Option<Stream>,
}

impl AudioModel {
    pub fn new(
        audio_stages: Vec<Box<dyn AudioStage<((), LiveParameterStorage)>>>,
        output_stream_params: (Device, StreamConfig, SampleFormat),
        options: AudioOptions,
        storage: LiveParameterStorage,
        storage_updates: Receiver<LiveParameterStorage>,
        audio_in: Producer<f64, Arc<HeapRb<f64>>>,
    ) -> Self {
        let (send, recv) = channel::unbounded();

        let sample_rate = output_stream_params.1.sample_rate;
        let audio_out = AudioOut {
            renderer: AudioRenderer {
                buffer: vec![0.0; usize::try_from(options.output_buffer_size).unwrap() * 4],
                audio_stages,
                storage,
                storage_updates,
                current_wav_writer: None,
                sample_rate_hz: sample_rate.0,
                wav_file_prefix: Arc::new(options.wav_file_prefix),
                updates: send.clone(),
            },
            updates: recv,
        };

        let audio_in = AudioIn {
            exchange_buffer: audio_in,
        };

        Self {
            output_stream: audio_out.start_stream(output_stream_params),
            input_stream: options
                .audio_in_enabled
                .then(|| audio_in.start_stream(options.input_buffer_size, sample_rate)),
        }
    }
}

struct AudioOut {
    renderer: AudioRenderer,
    updates: Receiver<UpdateFn>,
}

impl AudioOut {
    fn start_stream(
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
                    for update in self.updates.try_iter() {
                        update(&mut self.renderer);
                    }
                    self.renderer.render_audio(buffer);
                },
                |err| eprintln!("[ERROR] {err}"),
                None,
            )
            .unwrap()
    }
}

struct AudioRenderer {
    buffer: Vec<f64>,
    audio_stages: Vec<Box<dyn AudioStage<((), LiveParameterStorage)>>>,
    storage: LiveParameterStorage,
    storage_updates: Receiver<LiveParameterStorage>,
    current_wav_writer: Option<WavWriter<BufWriter<File>>>,
    sample_rate_hz: u32,
    wav_file_prefix: Arc<String>,
    updates: Sender<UpdateFn>,
}

impl AudioRenderer {
    fn render_audio<T: Sample + FromSample<f64>>(&mut self, buffer: &mut [T])
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

        let buffer_f64 = &mut self.buffer[0..buffer.len()];

        for sample in &mut *buffer_f64 {
            *sample = 0.0;
        }

        let context = AutomationContext {
            render_window_secs: buffer.len() as f64 / self.sample_rate_hz as f64,
            payload: &((), self.storage),
        };
        for audio_stage in &mut self.audio_stages {
            audio_stage.render(buffer_f64, &context);
        }

        for (src, dst) in buffer_f64.iter().zip(buffer.iter_mut()) {
            *dst = T::from_sample(*src);
        }

        if let Some(wav_writer) = &mut self.current_wav_writer {
            for &sample in &*buffer {
                wav_writer.write_sample(f32::from_sample(sample)).unwrap();
            }
        }
    }

    fn set_recording_active(&self, recording_active: bool) {
        let updates = self.updates.clone();
        let sample_rate_hz = self.sample_rate_hz;
        let wav_file_prefix = self.wav_file_prefix.clone();
        thread::spawn(move || {
            if recording_active {
                let wav_writer = create_wav_writer(sample_rate_hz, &wav_file_prefix);
                send_update(&updates, move |renderer| {
                    renderer.current_wav_writer = Some(wav_writer);
                    for audio_stage in &mut renderer.audio_stages {
                        audio_stage.mute();
                    }
                })
            } else {
                send_update(&updates, |renderer| renderer.current_wav_writer = None);
            }
        });
    }
}

struct AudioIn {
    exchange_buffer: Producer<f64, Arc<HeapRb<f64>>>,
}

impl AudioIn {
    fn start_stream(self, input_buffer_size: u32, sample_rate: SampleRate) -> Stream {
        let device = cpal::default_host().default_input_device().unwrap();
        let default_config = device.default_input_config().unwrap();
        let used_config = create_stream_config(
            "input",
            &default_config,
            input_buffer_size,
            Some(sample_rate),
        );
        let sample_format = default_config.sample_format();
        let stream = match sample_format {
            SampleFormat::F32 => self.create_stream::<f32>(&device, &used_config),
            SampleFormat::I16 => self.create_stream::<i16>(&device, &used_config),
            _ => panic!("Unsupported sample format {sample_format}"),
        };
        stream.play().unwrap();
        stream
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
    println!("[DEBUG] Default {stream_type} stream config:\n{default_config:#?}");
    let buffer_size = match default_config.buffer_size() {
        SupportedBufferSize::Range { .. } => BufferSize::Fixed(buffer_size),
        SupportedBufferSize::Unknown => {
            println!("[WARNING] Cannot set buffer size on {stream_type} audio device. The device's default buffer size will be used.");
            BufferSize::Default
        }
    };

    StreamConfig {
        channels: 2,
        sample_rate: sample_rate.unwrap_or_else(|| default_config.sample_rate()),
        buffer_size,
    }
}

fn create_wav_writer(sample_rate_hz: u32, file_prefix: &str) -> WavWriter<BufWriter<File>> {
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

    println!("[INFO] Created `{output_file_name}`");
    WavWriter::create(output_file_name, spec).unwrap()
}

fn send_update(
    updates: &Sender<UpdateFn>,
    update_fn: impl FnOnce(&mut AudioRenderer) + Send + 'static,
) {
    updates.send(Box::new(update_fn)).unwrap()
}

type UpdateFn = Box<dyn FnOnce(&mut AudioRenderer) + Send>;

pub trait AudioStage<T>: Send {
    fn render(&mut self, buffer: &mut [f64], context: &AutomationContext<T>);

    fn mute(&mut self);
}
