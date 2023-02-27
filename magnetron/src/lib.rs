pub mod automation;
pub mod buffer;
pub mod creator;
pub mod envelope;
pub mod waveform;

use automation::AutomationContext;
use buffer::{BufferWriter, WaveformBuffer};

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
    ) {
        self.process_internal(reset, num_samples, &mut [], payload, stages);
    }

    pub fn process_nested<'a, T>(
        &mut self,
        buffers: &mut BufferWriter,
        payload: &'a T,
        stages: impl IntoIterator<Item = &'a mut Stage<T>>,
    ) {
        self.process_internal(
            buffers.reset(),
            buffers.buffer_len(),
            buffers.internal_buffers(),
            payload,
            stages,
        );
    }

    fn process_internal<'a, T>(
        &mut self,
        reset: bool,
        num_samples: usize,
        external_buffers: &mut [WaveformBuffer],
        payload: &'a T,
        stages: impl IntoIterator<Item = &'a mut Stage<T>>,
    ) {
        for buffer in &mut self.buffers {
            buffer.set_dirty();
        }

        let mut buffer_writer = BufferWriter::new(
            self.sample_width_secs,
            external_buffers,
            &mut self.buffers,
            &self.zeros[..num_samples],
            reset,
        );

        let context = AutomationContext {
            render_window_secs: self.sample_width_secs * num_samples as f64,
            payload,
        };

        for stage in stages {
            stage.process(&mut buffer_writer, &context);
        }

        self.curr_size = num_samples;
    }

    pub fn read_buffer(&self, index: usize) -> &[f64] {
        self.buffers[index].read(&self.zeros[..self.curr_size])
    }
}

pub struct Stage<T> {
    state: StageState,
    stage_fn: StageFn<T>,
}

impl<T> Stage<T> {
    pub fn new(
        stage_fn: impl FnMut(&mut BufferWriter, &AutomationContext<T>) -> StageState + Send + 'static,
    ) -> Self {
        Self {
            state: StageState::Active,
            stage_fn: Box::new(stage_fn),
        }
    }

    pub fn process(&mut self, buffers: &mut BufferWriter, context: &AutomationContext<T>) {
        self.state = (self.stage_fn)(buffers, context);
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
