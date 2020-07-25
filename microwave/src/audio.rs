use crate::{effects::Delay, fluid::FluidSynth, synth::WaveformSynth};
use nannou_audio::{stream, Buffer, Host, Stream};
use std::hash::Hash;

pub struct AudioModel<E> {
    // This code isn't dead, actually. Audio is processed as long as the stream is alive.
    #[allow(dead_code)]
    stream: Stream<AudioRenderer<E>>,
}

struct AudioRenderer<E> {
    waveform_synth: WaveformSynth<E>,
    fluid_synth: FluidSynth,
    delay: Delay,
}

impl<E: 'static + Eq + Hash + Send> AudioModel<E> {
    pub fn new(
        fluid_synth: FluidSynth,
        waveform_synth: WaveformSynth<E>,
        buffer_size: usize,
        delay_secs: f32,
        delay_feedback: f32,
    ) -> Self {
        let renderer = AudioRenderer {
            waveform_synth,
            fluid_synth,
            delay: Delay::new(
                (delay_secs * (stream::DEFAULT_SAMPLE_RATE * 2) as f32).round() as usize,
                delay_feedback,
            ),
        };

        let stream = Host::new()
            .new_output_stream(renderer)
            .frames_per_buffer(buffer_size)
            .render(render_audio)
            .build()
            .unwrap();

        Self { stream }
    }
}

fn render_audio<E: Eq + Hash>(renderer: &mut AudioRenderer<E>, buffer: &mut Buffer) {
    renderer.fluid_synth.write(buffer);
    renderer.waveform_synth.write(buffer);
    renderer.delay.process(&mut buffer[..])
}
