use std::{fs::File, hash::Hash, io::BufWriter};

use chrono::Local;
use hound::{SampleFormat, WavSpec, WavWriter};
use nannou_audio::{stream, Buffer, Host, Stream};

use crate::{
    effects::{Delay, DelayOptions, Rotary, RotaryOptions},
    fluid::FluidSynth,
    synth::WaveformSynth,
};

pub struct AudioModel<E> {
    stream: Stream<AudioRenderer<E>>,
}

struct AudioRenderer<E> {
    waveform_synth: WaveformSynth<E>,
    fluid_synth: FluidSynth,
    delay: (Delay, bool),
    rotary: (Rotary, bool),
    current_recording: Option<WavWriter<BufWriter<File>>>,
}

impl<E: 'static + Eq + Hash + Send> AudioModel<E> {
    pub fn new(
        fluid_synth: FluidSynth,
        waveform_synth: WaveformSynth<E>,
        buffer_size: usize,
        delay_options: DelayOptions,
        rotary_options: RotaryOptions,
    ) -> Self {
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
        };

        let stream = Host::new()
            .new_output_stream(renderer)
            .frames_per_buffer(buffer_size)
            .render(render_audio)
            .build()
            .unwrap();

        Self { stream }
    }

    pub fn set_delay_active(&self, delay_active: bool) {
        self.stream
            .send(move |renderer| {
                renderer.delay.1 = delay_active;
                if !delay_active {
                    renderer.delay.0.mute();
                }
            })
            .unwrap();
    }

    pub fn set_rotary_active(&self, rotary_active: bool) {
        self.stream
            .send(move |renderer| {
                renderer.rotary.1 = rotary_active;
                if !rotary_active {
                    renderer.rotary.0.mute();
                }
            })
            .unwrap();
    }

    pub fn set_rotary_motor_voltage(&self, motor_voltage: f32) {
        self.stream
            .send(move |renderer| renderer.rotary.0.set_motor_voltage(motor_voltage))
            .unwrap();
    }

    pub fn set_recording_active(&self, recording_active: bool) {
        self.stream
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
    renderer.waveform_synth.write(buffer);
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
