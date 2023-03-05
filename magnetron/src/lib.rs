pub mod automation;
pub mod buffer;
pub mod creator;
pub mod envelope;
pub mod waveform;

use std::{iter, sync::Arc};

use automation::AutomationContext;
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

    pub fn write<T>(&mut self, waveform: &mut Waveform<T>, payload: &T) {
        let buffers = &mut self.buffers;

        let len = buffers.readable.mix.len;
        for buffer in &mut buffers.readable.intermediate {
            buffer.clear(len);
        }
        buffers.readable.audio_out.clear(len);

        let render_window_secs = buffers.sample_width_secs * len as f64;
        let context = AutomationContext {
            render_window_secs,
            payload,
        };

        for stage in &mut waveform.stages {
            stage.render(buffers, &context);
        }
        waveform.is_active = waveform.envelope.render(buffers, &context).is_active();
    }

    pub fn mix(&self) -> &[f64] {
        self.buffers.readable.mix.read()
    }
}

pub struct Stage<T> {
    pub(crate) stage_fn: StageFn<T>,
}

impl<T> Stage<T> {
    pub fn render(
        &mut self,
        buffers: &mut BufferWriter,
        context: &AutomationContext<T>,
    ) -> StageState {
        (self.stage_fn)(buffers, context)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageState {
    Active,
    Exhausted,
}

impl StageState {
    pub fn is_active(&self) -> bool {
        match self {
            StageState::Active => true,
            StageState::Exhausted => false,
        }
    }
}

type StageFn<T> = Box<dyn FnMut(&mut BufferWriter, &AutomationContext<T>) -> StageState + Send>;
