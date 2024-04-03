//! Building blocks for constructing audio processing pipelines.

use crate::{automation::AutomationInfo, buffer::BufferWriter};

/// A basic building block of an audio processing pipeline that can read and/or write data from/to an audio buffer.
pub struct Stage<A: AutomationInfo> {
    stage_fn: StageFn<A>,
}

type StageFn<A> =
    Box<dyn FnMut(&mut BufferWriter, <A as AutomationInfo>::Context<'_>) -> StageActivity + Send>;

impl<A: AutomationInfo> Stage<A> {
    pub fn new(
        stage_fn: impl FnMut(&mut BufferWriter, A::Context<'_>) -> StageActivity + Send + 'static,
    ) -> Self {
        Self {
            stage_fn: Box::new(stage_fn),
        }
    }

    pub fn process(
        &mut self,
        buffers: &mut BufferWriter,
        context: A::Context<'_>,
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
