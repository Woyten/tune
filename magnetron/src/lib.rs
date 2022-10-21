pub mod automation;
pub mod buffer;
pub mod spec;
pub mod waveform;

use std::{iter, sync::Arc};

use automation::{AutomationContext, AutomationSpec};
use buffer::{BufferWriter, ReadableBuffers, WaveformBuffer};
use waveform::Waveform;

pub struct Magnetron {
    buffers: BufferWriter,
}

impl Magnetron {
    pub fn new(sample_width_secs: f64, num_buffers: usize, buffer_size: usize) -> Self {
        let zeros = Arc::<[f64]>::from(vec![0.0; buffer_size]);
        Self {
            buffers: BufferWriter {
                sample_width_secs,
                readable: ReadableBuffers {
                    audio_in: WaveformBuffer::new(zeros.clone()),
                    intermediate: vec![WaveformBuffer::new(zeros.clone()); num_buffers],
                    audio_out: WaveformBuffer::new(zeros.clone()),
                    mix: WaveformBuffer::new(zeros.clone()),
                },
                writeable: WaveformBuffer::new(zeros), // Empty Vec acting as a placeholder
            },
        }
    }

    pub fn clear(&mut self, len: usize) {
        self.buffers.readable.audio_in.clear(len);
        self.buffers.readable.mix.clear(len);
    }

    pub fn set_audio_in(&mut self, mut buffer_content: impl FnMut() -> f64) {
        self.buffers
            .readable
            .audio_in
            .write(iter::from_fn(|| Some(buffer_content())));
    }

    pub fn write<A: AutomationSpec>(
        &mut self,
        waveform: &mut Waveform<A>,
        payload: &A::Context,
        note_suspension: f64,
    ) {
        let buffers = &mut self.buffers;

        let len = buffers.readable.mix.len;
        for buffer in &mut buffers.readable.intermediate {
            buffer.clear(len);
        }
        buffers.readable.audio_out.clear(len);

        let state = &mut waveform.state;

        let render_window_secs = buffers.sample_width_secs * len as f64;
        let context = AutomationContext {
            render_window_secs,
            payload,
        };

        for stage in &mut waveform.stages {
            stage.render(buffers, &context);
        }

        let out_buffer = buffers.readable.audio_out.read();

        let from_amplitude = waveform
            .envelope
            .get_value(state.secs_since_pressed, state.secs_since_released);

        state.secs_since_pressed += render_window_secs;
        state.secs_since_released += render_window_secs * (1.0 - note_suspension);

        let to_amplitude = waveform
            .envelope
            .get_value(state.secs_since_pressed, state.secs_since_released);

        let mut curr_amplitude = from_amplitude;
        let slope = (to_amplitude - from_amplitude) / len as f64;

        buffers.readable.mix.write(out_buffer.iter().map(|src| {
            let result = src * curr_amplitude * state.velocity;
            curr_amplitude = (curr_amplitude + slope).clamp(0.0, 1.0);
            result
        }));
    }

    pub fn mix(&self) -> &[f64] {
        self.buffers.readable.mix.read()
    }
}
