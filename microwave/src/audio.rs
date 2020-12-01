use std::{fs::File, hash::Hash, io::BufWriter};

use chrono::Local;
use hound::{SampleFormat, WavSpec, WavWriter};
use nannou_audio::{stream, Buffer, Host, Stream};
use ringbuf::{Consumer, Producer, RingBuffer};

use crate::{
    effects::{Delay, DelayOptions, Rotary, RotaryOptions},
    fluid::FluidSynth,
    synth::WaveformSynth,
};

pub struct AudioModel<E> {
    output_stream: Stream<AudioRenderer<E>>,
    // Not dead, actually. Audio-in is active as long as this Stream is not dropped.
    #[allow(dead_code)]
    input_stream: Option<Stream<Producer<f32>>>,
}

struct AudioRenderer<E> {
    waveform_synth: WaveformSynth<E>,
    fluid_synth: FluidSynth,
    delay: (Delay, bool),
    rotary: (Rotary, bool),
    current_recording: Option<WavWriter<BufWriter<File>>>,
    audio_in: Consumer<f32>,
}

impl<E: 'static + Eq + Hash + Send> AudioModel<E> {
    pub fn new(
        fluid_synth: FluidSynth,
        waveform_synth: WaveformSynth<E>,
        options: AudioOptions,
        delay_options: DelayOptions,
        rotary_options: RotaryOptions,
    ) -> Self {
        let (prod, cons) = RingBuffer::new(options.exchange_buffer_size * 2).split();

        let renderer = AudioRenderer {
            waveform_synth,
            fluid_synth,
            delay: (
                Delay::new(delay_options, stream::DEFAULT_SAMPLE_RATE as f32),
                true,
            ),
            rotary: (
                Rotary::new(rotary_options, stream::DEFAULT_SAMPLE_RATE as f32),
                false,
            ),
            current_recording: None,
            audio_in: cons,
        };

        let output_stream = Host::new()
            .new_output_stream(renderer)
            .frames_per_buffer(options.output_buffer_size)
            .render(render_audio)
            .build()
            .unwrap();

        let input_stream = match options.audio_in_enabled {
            true => Some(
                Host::new()
                    .new_input_stream(prod)
                    .frames_per_buffer(options.input_buffer_size)
                    .capture(|prod: &mut Producer<f32>, buffer: &Buffer| {
                        prod.push_iter(&mut buffer[..].iter().copied());
                    })
                    .build()
                    .unwrap(),
            ),
            false => None,
        };

        Self {
            output_stream,
            input_stream,
        }
    }

    pub fn set_delay_active(&self, delay_active: bool) {
        self.output_stream
            .send(move |renderer| {
                renderer.delay.1 = delay_active;
                if !delay_active {
                    renderer.delay.0.mute();
                }
            })
            .unwrap();
    }

    pub fn set_rotary_active(&self, rotary_active: bool) {
        self.output_stream
            .send(move |renderer| {
                renderer.rotary.1 = rotary_active;
                if !rotary_active {
                    renderer.rotary.0.mute();
                }
            })
            .unwrap();
    }

    pub fn set_rotary_motor_voltage(&self, motor_voltage: f32) {
        self.output_stream
            .send(move |renderer| renderer.rotary.0.set_motor_voltage(motor_voltage))
            .unwrap();
    }

    pub fn set_recording_active(&self, recording_active: bool) {
        self.output_stream
            .send(move |renderer| {
                if recording_active {
                    renderer.current_recording = Some(create_writer());
                    renderer.delay.0.mute();
                    renderer.rotary.0.mute();
                } else {
                    renderer.current_recording = None
                }
            })
            .unwrap();
    }
}

pub struct AudioOptions {
    pub audio_in_enabled: bool,
    pub output_buffer_size: usize,
    pub input_buffer_size: usize,
    pub exchange_buffer_size: usize,
}

fn create_writer() -> WavWriter<BufWriter<File>> {
    let output_file_name = format!("microwave_{}.wav", Local::now().format("%Y%m%d_%H%M%S"));
    let spec = WavSpec {
        channels: 2,
        sample_rate: stream::DEFAULT_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    println!("[INFO] Created `{}`", output_file_name);
    WavWriter::create(output_file_name, spec).unwrap()
}

fn render_audio<E: Eq + Hash>(renderer: &mut AudioRenderer<E>, buffer: &mut Buffer) {
    renderer.fluid_synth.write(buffer);
    renderer
        .waveform_synth
        .write(buffer, &mut renderer.audio_in);

    if renderer.rotary.1 {
        renderer.rotary.0.process(&mut buffer[..]);
    }
    if renderer.delay.1 {
        renderer.delay.0.process(&mut buffer[..]);
    }
    if let Some(wav_writer) = &mut renderer.current_recording {
        for &sample in &buffer[..] {
            wav_writer.write_sample(sample).unwrap();
        }
    }
}
