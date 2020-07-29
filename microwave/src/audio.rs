use crate::{effects::Delay, fluid::FluidSynth, synth::WaveformSynth};
use chrono::Local;
use hound::{SampleFormat, WavSpec, WavWriter};
use nannou_audio::{stream, Buffer, Host, Stream};
use std::{fs::File, hash::Hash, io::BufWriter};

pub struct AudioModel<E> {
    stream: Stream<AudioRenderer<E>>,
}

struct AudioRenderer<E> {
    waveform_synth: WaveformSynth<E>,
    fluid_synth: FluidSynth,
    delay: Delay,
    current_recording: Option<WavWriter<BufWriter<File>>>,
}

impl<E: 'static + Eq + Hash + Send> AudioModel<E> {
    pub fn new(
        fluid_synth: FluidSynth,
        waveform_synth: WaveformSynth<E>,
        buffer_size: usize,
        delay_secs: f32,
        delay_feedback: f32,
        delay_feedback_rotation_radians: f32,
    ) -> Self {
        let renderer = AudioRenderer {
            waveform_synth,
            fluid_synth,
            delay: Delay::new(
                (delay_secs * stream::DEFAULT_SAMPLE_RATE as f32).round() as usize,
                delay_feedback,
                delay_feedback_rotation_radians,
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

    pub fn start_recording(&self) {
        self.stream
            .send(move |renderer| {
                renderer.current_recording = Some(create_writer());
                renderer.delay.mute()
            })
            .unwrap();
    }

    pub fn stop_recording(&self) {
        self.stream
            .send(move |renderer| renderer.current_recording = None)
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
    renderer.delay.process(&mut buffer[..]);
    if let Some(wav_writer) = &mut renderer.current_recording {
        for &sample in &buffer[..] {
            wav_writer.write_sample(sample).unwrap();
        }
    }
}
