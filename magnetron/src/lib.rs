pub mod automation;
pub mod buffer;
pub mod creator;
pub mod envelope;
pub mod stage;

use automation::AutomationContext;
use buffer::{BufferWriter, WaveformBuffer};
use stage::{Stage, StageActivity};

/// Main component for driving an audio processing pipeline.
pub struct Magnetron {
    curr_size: usize,
    sample_width_secs: f64,
    buffers: Vec<WaveformBuffer>,
    zeros: Vec<f64>,
}

impl Magnetron {
    pub fn new(sample_width_secs: f64, num_buffers: usize, buffer_size: usize) -> Self {
        Self {
            curr_size: 0,
            sample_width_secs,
            buffers: vec![WaveformBuffer::new(buffer_size); num_buffers],
            zeros: vec![0.0; buffer_size],
        }
    }

    pub fn process<'a, T>(
        &mut self,
        reset: bool,
        num_samples: usize,
        payload: &'a T,
        stages: impl IntoIterator<Item = &'a mut Stage<T>>,
    ) -> StageActivity {
        self.process_internal(reset, num_samples, &mut [], payload, stages)
    }

    pub fn process_nested<'a, T>(
        &mut self,
        buffers: &mut BufferWriter,
        payload: &'a T,
        stages: impl IntoIterator<Item = &'a mut Stage<T>>,
    ) -> StageActivity {
        self.process_internal(
            buffers.reset(),
            buffers.buffer_len(),
            buffers.internal_buffers(),
            payload,
            stages,
        )
    }

    fn process_internal<'a, T>(
        &mut self,
        reset: bool,
        num_samples: usize,
        external_buffers: &mut [WaveformBuffer],
        payload: &'a T,
        stages: impl IntoIterator<Item = &'a mut Stage<T>>,
    ) -> StageActivity {
        self.curr_size = num_samples;
        for buffer in &mut self.buffers {
            buffer.set_dirty();
        }

        let mut buffer_writer = BufferWriter::new(
            self.sample_width_secs,
            external_buffers,
            &mut self.buffers,
            &self.zeros[..self.curr_size],
            reset,
        );

        let context = AutomationContext {
            render_window_secs: self.sample_width_secs * self.curr_size as f64,
            payload,
        };

        stages
            .into_iter()
            .map(|stage| stage.process(&mut buffer_writer, &context))
            .max()
            .unwrap_or_default()
    }

    pub fn read_buffer(&self, index: usize) -> &[f64] {
        self.buffers[index].read(&self.zeros[..self.curr_size])
    }
}
