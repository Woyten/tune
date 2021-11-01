use std::{
    fs::File,
    hash::Hash,
    io::BufWriter,
    sync::mpsc::{self, Receiver, Sender},
};

use chrono::Local;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, Device, Sample, SampleFormat, SampleRate, Stream, StreamConfig,
    SupportedBufferSize, SupportedStreamConfig,
};
use fluid_xenth::fluidlite::IsSamples;
use hound::{WavSpec, WavWriter};
use ringbuf::{Consumer, Producer, RingBuffer};

use crate::{
    fluid::FluidSynth,
    magnetron::effects::{
        Delay, DelayOptions, ReverbOptions, Rotary, RotaryOptions, SchroederReverb,
    },
    synth::WaveformSynth,
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

pub struct AudioModel<S> {
    // Not dead, actually. Audio-out is active as long as this Stream is not dropped.
    #[allow(dead_code)]
    output_stream: Stream,
    // Not dead, actually. Audio-in is active as long as this Stream is not dropped.
    #[allow(dead_code)]
    input_stream: Option<Stream>,
    updates: Sender<UpdateFn<S>>,
    sample_rate_hz: u32,
    wav_file_prefix: String,
}

impl<S: Eq + Hash + Send + 'static> AudioModel<S> {
    pub fn new(
        fluid_synth: FluidSynth,
        waveform_synth: WaveformSynth<S>,
        output_stream_params: (Device, StreamConfig, SampleFormat),
        options: AudioOptions,
        reverb_options: ReverbOptions,
        delay_options: DelayOptions,
        rotary_options: RotaryOptions,
    ) -> Self {
        let (send, recv) = mpsc::channel();
        let (prod, cons) = RingBuffer::new(options.exchange_buffer_size * 2).split();

        let sample_rate = output_stream_params.1.sample_rate;
        let sample_rate_hz_u32 = sample_rate.0;
        let sample_rate_hz_f64 = f64::from(sample_rate_hz_u32);

        let audio_out = AudioOut {
            renderer: AudioRenderer {
                buffer: vec![0.0; usize::try_from(options.output_buffer_size).unwrap() * 4],
                waveform_synth,
                fluid_synth,
                reverb: (
                    SchroederReverb::new(reverb_options, sample_rate_hz_f64),
                    false,
                ),
                delay: (Delay::new(delay_options, sample_rate_hz_f64), false),
                rotary: (Rotary::new(rotary_options, sample_rate_hz_f64), false),
                current_wav_writer: None,
                exchange_buffer: cons,
            },
            updates: recv,
        };

        let audio_in = AudioIn {
            exchange_buffer: prod,
        };

        Self {
            output_stream: audio_out.start_stream(output_stream_params),
            input_stream: options
                .audio_in_enabled
                .then(|| audio_in.start_stream(options.input_buffer_size, sample_rate)),
            updates: send,
            sample_rate_hz: sample_rate_hz_u32,
            wav_file_prefix: options.wav_file_prefix,
        }
    }

    pub fn set_reverb_active(&self, reverb_active: bool) {
        self.update(move |renderer| {
            renderer.reverb.1 = reverb_active;
            if !reverb_active {
                renderer.reverb.0.mute();
            }
        });
    }

    pub fn set_delay_active(&self, delay_active: bool) {
        self.update(move |renderer| {
            renderer.delay.1 = delay_active;
            if !delay_active {
                renderer.delay.0.mute();
            }
        });
    }

    pub fn set_rotary_active(&self, rotary_active: bool) {
        self.update(move |renderer| {
            renderer.rotary.1 = rotary_active;
            if !rotary_active {
                renderer.rotary.0.mute();
            }
        });
    }

    pub fn set_rotary_motor_voltage(&self, motor_voltage: f64) {
        self.update(move |renderer| renderer.rotary.0.set_motor_voltage(motor_voltage));
    }

    pub fn set_recording_active(&self, recording_active: bool) {
        if recording_active {
            let wav_writer = create_wav_writer(self.sample_rate_hz, &self.wav_file_prefix);
            self.update(move |renderer| {
                renderer.current_wav_writer = Some(wav_writer);
                renderer.reverb.0.mute();
                renderer.delay.0.mute();
                renderer.rotary.0.mute();
            })
        } else {
            self.update(|renderer| renderer.current_wav_writer = None);
        }
    }

    fn update(&self, update_fn: impl FnOnce(&mut AudioRenderer<S>) + Send + 'static) {
        self.updates.send(Box::new(update_fn)).unwrap()
    }
}

struct AudioOut<S> {
    renderer: AudioRenderer<S>,
    updates: Receiver<UpdateFn<S>>,
}

impl<S: Eq + Hash + Send + 'static> AudioOut<S> {
    fn start_stream(
        self,
        (device, stream_config, sample_format): (Device, StreamConfig, SampleFormat),
    ) -> Stream {
        let stream = match sample_format {
            SampleFormat::F32 => self.create_stream::<f32>(&device, &stream_config),
            SampleFormat::I16 => self.create_stream::<i16>(&device, &stream_config),
            SampleFormat::U16 => panic!("U16 sample format not supported"),
        };
        stream.play().unwrap();
        stream
    }

    fn create_stream<T: Sample>(mut self, device: &Device, config: &StreamConfig) -> Stream
    where
        for<'a> &'a mut [T]: IsSamples,
    {
        device
            .build_output_stream(
                config,
                move |buffer, _| {
                    for update in self.updates.try_iter() {
                        update(&mut self.renderer);
                    }
                    self.renderer.render_audio(buffer);
                },
                |_| {},
            )
            .unwrap()
    }
}

struct AudioRenderer<S> {
    buffer: Vec<f64>,
    waveform_synth: WaveformSynth<S>,
    fluid_synth: FluidSynth,
    reverb: (SchroederReverb, bool),
    delay: (Delay, bool),
    rotary: (Rotary, bool),
    current_wav_writer: Option<WavWriter<BufWriter<File>>>,
    exchange_buffer: Consumer<f64>,
}

impl<S: Eq + Hash> AudioRenderer<S> {
    fn render_audio<T: Sample>(&mut self, buffer: &mut [T])
    where
        for<'a> &'a mut [T]: IsSamples,
    {
        let buffer_f64 = &mut self.buffer[0..buffer.len()];

        self.fluid_synth.write(&mut *buffer);
        for (src, dst) in buffer.iter().zip(buffer_f64.iter_mut()) {
            *dst = f64::from(src.to_f32());
        }
        self.waveform_synth
            .write(buffer_f64, &mut self.exchange_buffer);

        if self.rotary.1 {
            self.rotary.0.process(buffer_f64);
        }
        if self.reverb.1 {
            self.reverb.0.process(buffer_f64);
        }
        if self.delay.1 {
            self.delay.0.process(buffer_f64);
        }

        for (src, dst) in buffer_f64.iter().zip(buffer.iter_mut()) {
            *dst = T::from(&(*src as f32));
        }

        if let Some(wav_writer) = &mut self.current_wav_writer {
            for &sample in &*buffer {
                wav_writer.write_sample(sample.to_f32()).unwrap();
            }
        }
    }
}

struct AudioIn {
    exchange_buffer: Producer<f64>,
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
        let stream = match default_config.sample_format() {
            SampleFormat::F32 => self.create_stream::<f32>(&device, &used_config),
            SampleFormat::I16 => self.create_stream::<i16>(&device, &used_config),
            SampleFormat::U16 => panic!("U16 sample format not supported"),
        };
        stream.play().unwrap();
        stream
    }

    fn create_stream<T: Sample>(mut self, device: &Device, config: &StreamConfig) -> Stream {
        device
            .build_input_stream(
                config,
                move |buffer: &[T], _| {
                    self.exchange_buffer
                        .push_iter(&mut buffer[..].iter().map(|&s| f64::from(s.to_f32())));
                },
                |_| {},
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
    println!(
        "[DEBUG] Default {} stream config:\n{:#?}",
        stream_type, default_config
    );
    let buffer_size = match default_config.buffer_size() {
        SupportedBufferSize::Range { .. } => BufferSize::Fixed(buffer_size),
        SupportedBufferSize::Unknown => {
            println!("[WARNING] Cannot set buffer size on {} audio device. The device's default buffer size will be used.", stream_type);
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

    println!("[INFO] Created `{}`", output_file_name);
    WavWriter::create(output_file_name, spec).unwrap()
}

type UpdateFn<S> = Box<dyn FnOnce(&mut AudioRenderer<S>) + Send>;
