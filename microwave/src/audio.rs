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
    effects::{Delay, DelayOptions, ReverbOptions, Rotary, RotaryOptions, SchroederReverb},
    fluid::FluidSynth,
    synth::WaveformSynth,
};

pub const DEFAULT_SAMPLE_RATE: u32 = 44100;

pub struct AudioModel<E> {
    // Not dead, actually. Audio-out is active as long as this Stream is not dropped.
    #[allow(dead_code)]
    output_stream: Stream,
    // Not dead, actually. Audio-in is active as long as this Stream is not dropped.
    #[allow(dead_code)]
    input_stream: Option<Stream>,
    updates: Sender<UpdateFn<E>>,
}

type UpdateFn<E> = Box<dyn FnMut(&mut AudioRenderer<E>) + Send>;

struct AudioRenderer<E> {
    waveform_synth: WaveformSynth<E>,
    fluid_synth: FluidSynth,
    reverb: (SchroederReverb, bool),
    delay: (Delay, bool),
    rotary: (Rotary, bool),
    current_recording: Option<WavWriter<BufWriter<File>>>,
    audio_in: Consumer<f32>,
}

impl<E: Eq + Hash + Send + 'static> AudioModel<E> {
    pub fn new(
        fluid_synth: FluidSynth,
        waveform_synth: WaveformSynth<E>,
        options: AudioOptions,
        reverb_options: ReverbOptions,
        delay_options: DelayOptions,
        rotary_options: RotaryOptions,
    ) -> Self {
        let (mut prod, cons) = RingBuffer::new(options.exchange_buffer_size * 2).split();
        let (send, recv) = mpsc::channel::<UpdateFn<E>>();

        let mut renderer = AudioRenderer {
            waveform_synth,
            fluid_synth,
            reverb: (
                SchroederReverb::new(reverb_options, DEFAULT_SAMPLE_RATE as f32),
                false,
            ),
            delay: (Delay::new(delay_options, DEFAULT_SAMPLE_RATE as f32), false),
            rotary: (
                Rotary::new(rotary_options, DEFAULT_SAMPLE_RATE as f32),
                false,
            ),
            current_recording: None,
            audio_in: cons,
        };

        let output_device = cpal::default_host().default_output_device().unwrap();

        let output_config = StreamConfig {
            channels: 2,
            sample_rate: SampleRate(DEFAULT_SAMPLE_RATE),
            buffer_size: BufferSize::Fixed(options.output_buffer_size),
        };

        let output_stream = output_device
            .build_output_stream(
                &output_config,
                move |buffer, _| {
                    for mut update in recv.try_iter() {
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
                    sample_rate: SampleRate(DEFAULT_SAMPLE_RATE),
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

    pub fn set_rotary_motor_voltage(&self, motor_voltage: f32) {
        self.update(move |renderer| renderer.rotary.0.set_motor_voltage(motor_voltage));
    }

    pub fn set_recording_active(&self, recording_active: bool) {
        self.update(move |renderer| {
            if recording_active {
                renderer.current_recording = Some(create_writer());
                renderer.reverb.0.mute();
                renderer.delay.0.mute();
                renderer.rotary.0.mute();
            } else {
                renderer.current_recording = None
            }
        });
    }

    fn update(&self, update_fn: impl Fn(&mut AudioRenderer<E>) + Send + 'static) {
        self.updates.send(Box::new(update_fn)).unwrap()
    }
}

pub struct AudioOptions {
    pub audio_in_enabled: bool,
    pub output_buffer_size: u32,
    pub input_buffer_size: u32,
    pub exchange_buffer_size: usize,
}

fn create_writer() -> WavWriter<BufWriter<File>> {
    let output_file_name = format!("microwave_{}.wav", Local::now().format("%Y%m%d_%H%M%S"));
    let spec = WavSpec {
        channels: 2,
        sample_rate: DEFAULT_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    println!("[INFO] Created `{}`", output_file_name);
    WavWriter::create(output_file_name, spec).unwrap()
}

impl<E: Eq + Hash> AudioRenderer<E> {
    fn render_audio(&mut self, buffer: &mut [f32]) {
        self.fluid_synth.write(buffer);
        self.waveform_synth.write(buffer, &mut self.audio_in);

        if self.rotary.1 {
            self.rotary.0.process(&mut buffer[..]);
        }
        if self.reverb.1 {
            self.reverb.0.process(&mut buffer[..]);
        }
        if self.delay.1 {
            self.delay.0.process(&mut buffer[..]);
        }
        if let Some(wav_writer) = &mut self.current_recording {
            for &sample in &buffer[..] {
                wav_writer.write_sample(sample).unwrap();
            }
        }
    }
}
