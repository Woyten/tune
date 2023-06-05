//! Building blocks for constructing audio processing pipelines.

use crate::{automation::QueryInfo, buffer::BufferWriter};

/// A basic building block of an audio processing pipeline that can read and/or write data from/to an audio buffer.
pub struct Stage<Q: QueryInfo> {
    stage_fn: StageFn<Q>,
}

type StageFn<Q> =
    Box<dyn FnMut(&mut BufferWriter, <Q as QueryInfo>::Context<'_>) -> StageActivity + Send>;

impl<Q: QueryInfo> Stage<Q> {
    pub fn new(
        stage_fn: impl FnMut(&mut BufferWriter, Q::Context<'_>) -> StageActivity + Send + 'static,
    ) -> Self {
        Self {
            stage_fn: Box::new(stage_fn),
        }
    }

    pub fn process(
        &mut self,
        buffers: &mut BufferWriter,
        context: Q::Context<'_>,
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
