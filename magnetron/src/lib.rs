pub mod automation;
pub mod buffer;
pub mod creator;
pub mod envelope;
pub mod stage;

use buffer::{BufferWriter, WaveformBuffer};

/// Main component for driving an audio processing pipeline.
pub struct Magnetron {
    sample_width_secs: f64,
    buffers: Vec<WaveformBuffer>,
    zeros: Vec<f64>,
}

impl Magnetron {
    pub fn new(sample_width_secs: f64, num_buffers: usize, max_buffer_size: usize) -> Self {
        Self {
            sample_width_secs,
            buffers: vec![WaveformBuffer::new(max_buffer_size); num_buffers],
            zeros: vec![0.0; max_buffer_size],
        }
    }

    pub fn prepare(&mut self, num_samples: usize, reset: bool) -> BufferWriter {
        self.prepare_internal(num_samples, &mut [], reset)
    }

    pub fn prepare_nested<'a>(&'a mut self, buffers: &'a mut BufferWriter) -> BufferWriter {
        let reset = buffers.reset();
        self.prepare_internal(buffers.buffer_len(), buffers.internal_buffers(), reset)
    }

    fn prepare_internal<'a>(
        &'a mut self,
        num_samples: usize,
        external_buffers: &'a mut [WaveformBuffer],
        reset: bool,
    ) -> BufferWriter {
        for buffer in self.buffers.iter_mut() {
            buffer.set_dirty();
        }

        BufferWriter::new(self, num_samples, external_buffers, reset)
    }
}
