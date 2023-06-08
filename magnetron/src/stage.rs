//! Building blocks for the construction of audio processing pipelines.

use crate::{automation::AutomationContext, buffer::BufferWriter};

/// A basic building block of an audio processing pipeline that can read and/or write data from/to an audio buffer.
pub struct Stage<T> {
    stage_fn: StageFn<T>,
}

type StageFn<T> = Box<dyn FnMut(&mut BufferWriter, &AutomationContext<T>) -> StageActivity + Send>;

impl<T> Stage<T> {
    pub fn new(
        stage_fn: impl FnMut(&mut BufferWriter, &AutomationContext<T>) -> StageActivity + Send + 'static,
    ) -> Self {
        Self {
            stage_fn: Box::new(stage_fn),
        }
    }

    pub fn process(
        &mut self,
        buffers: &mut BufferWriter,
        context: &AutomationContext<T>,
    ) -> StageActivity {
        (self.stage_fn)(buffers, context)
    }
}

/// Enum describing whether a [`Stage`] will continue to create some observable output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum StageActivity {
    /// The stage will no longer provide any observable output.
    #[default]
    Exhausted,

    /// The stage might fill an internal buffer.
    Internal,

    /// The stage might fill an external buffer.
    External,

    /// The stage has some other side effect.
    Observer,
}
