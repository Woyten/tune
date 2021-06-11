use std::{
    fs::File,
    hash::Hash,
    io::BufWriter,
    sync::mpsc::{self, Sender},
};

use chrono::Local;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, SampleRate, Stream, StreamConfig,
};
use hound::{SampleFormat, WavSpec, WavWriter};
use ringbuf::{Consumer, RingBuffer};

use crate::{
    fluid::FluidSynth,
    magnetron::effects::{
        Delay, DelayOptions, ReverbOptions, Rotary, RotaryOptions, SchroederReverb,
    },
    synth::WaveformSynth,
};

const DEFAULT_SAMPLE_RATE_U32: u32 = 44100;
pub const DEFAULT_SAMPLE_RATE: f64 = DEFAULT_SAMPLE_RATE_U32 as f64;

pub struct AudioModel<S> {
    // Not dead, actually. Audio-out is active as long as this Stream is not dropped.
    #[allow(dead_code)]
    output_stream: Stream,
    // Not dead, actually. Audio-in is active as long as this Stream is not dropped.
    #[allow(dead_code)]
    input_stream: Option<Stream>,
    updates: Sender<UpdateFn<S>>,
    wav_file_prefix: String,
}

type UpdateFn<S> = Box<dyn FnOnce(&mut AudioRenderer<S>) + Send>;

struct AudioRenderer<S> {
    buffer: Vec<f64>,
    waveform_synth: WaveformSynth<S>,
    fluid_synth: FluidSynth,
    reverb: (SchroederReverb, bool),
    delay: (Delay, bool),
    rotary: (Rotary, bool),
    current_wav_writer: Option<WavWriter<BufWriter<File>>>,
    audio_in: Consumer<f32>,
}

impl<S: Eq + Hash + Send + 'static> AudioModel<S> {
    pub fn new(
        fluid_synth: FluidSynth,
        waveform_synth: WaveformSynth<S>,
        options: AudioOptions,
        reverb_options: ReverbOptions,
        delay_options: DelayOptions,
        rotary_options: RotaryOptions,
    ) -> Self {
        let (mut prod, cons) = RingBuffer::new(options.exchange_buffer_size * 2).split();
        let (send, recv) = mpsc::channel::<UpdateFn<S>>();

        let mut renderer = AudioRenderer {
            buffer: vec![0.0; options.output_buffer_size as usize * 4],
            waveform_synth,
            fluid_synth,
            reverb: (
                SchroederReverb::new(reverb_options, DEFAULT_SAMPLE_RATE),
                false,
            ),
            delay: (Delay::new(delay_options, DEFAULT_SAMPLE_RATE), false),
            rotary: (Rotary::new(rotary_options, DEFAULT_SAMPLE_RATE), false),
            current_wav_writer: None,
            audio_in: cons,
        };

        let output_device = cpal::default_host().default_output_device().unwrap();

        let output_config = StreamConfig {
            channels: 2,
            sample_rate: SampleRate(DEFAULT_SAMPLE_RATE_U32),
            buffer_size: BufferSize::Fixed(options.output_buffer_size),
        };

        let output_stream = output_device
            .build_output_stream(
                &output_config,
                move |buffer, _| {
                    for update in recv.try_iter() {
                        update(&mut renderer);
                    }
                    renderer.render_audio(buffer);
                },
                |_| {},
            )
            .unwrap();

        output_stream.play().unwrap();

        let input_stream = match options.audio_in_enabled {
            true => Some({
                let input_device = cpal::default_host().default_input_device().unwrap();

                let input_config = StreamConfig {
                    channels: 2,
                    sample_rate: SampleRate(DEFAULT_SAMPLE_RATE_U32),
                    buffer_size: BufferSize::Fixed(options.input_buffer_size),
                };

                let input_stream = input_device
                    .build_input_stream(
                        &input_config,
                        move |buffer, _| {
                            prod.push_iter(&mut buffer[..].iter().copied());
                        },
                        |_| {},
                    )
                    .unwrap();

                input_stream.play().unwrap();

                input_stream
            }),
            false => None,
        };

        Self {
            output_stream,
            input_stream,
            updates: send,
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
            let wav_writer = create_wav_writer(&self.wav_file_prefix);
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

pub struct AudioOptions {
    pub audio_in_enabled: bool,
    pub output_buffer_size: u32,
    pub input_buffer_size: u32,
    pub exchange_buffer_size: usize,
    pub wav_file_prefix: String,
}

fn create_wav_writer(file_prefix: &str) -> WavWriter<BufWriter<File>> {
    let output_file_name = format!(
        "{}_{}.wav",
        file_prefix,
        Local::now().format("%Y%m%d_%H%M%S")
    );
    let spec = WavSpec {
        channels: 2,
        sample_rate: DEFAULT_SAMPLE_RATE_U32,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    println!("[INFO] Created `{}`", output_file_name);
    WavWriter::create(output_file_name, spec).unwrap()
}

impl<S: Eq + Hash> AudioRenderer<S> {
    fn render_audio(&mut self, buffer: &mut [f32]) {
        let buffer_f32 = buffer;
        let buffer_f64 = &mut self.buffer[0..buffer_f32.len()];

        self.fluid_synth.write(buffer_f32);
        for (src, dst) in buffer_f32.iter().zip(buffer_f64.iter_mut()) {
            *dst = f64::from(*src);
        }
        self.waveform_synth.write(buffer_f64, &mut self.audio_in);

        if self.rotary.1 {
            self.rotary.0.process(buffer_f64);
        }
        if self.reverb.1 {
            self.reverb.0.process(buffer_f64);
        }
        if self.delay.1 {
            self.delay.0.process(buffer_f64);
        }

        for (src, dst) in buffer_f64.iter().zip(buffer_f32.iter_mut()) {
            *dst = *src as f32;
        }

        if let Some(wav_writer) = &mut self.current_wav_writer {
            for &sample in &buffer_f32[..] {
                wav_writer.write_sample(sample).unwrap();
            }
        }
    }
}
