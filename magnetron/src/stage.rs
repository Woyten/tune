//! Building blocks for constructing audio processing pipelines.

use crate::{automation::ContextInfo, buffer::BufferWriter};

/// A basic building block of an audio processing pipeline that can read and/or write data from/to an audio buffer.
pub struct Stage<C: ContextInfo> {
    stage_fn: StageFn<C>,
}

type StageFn<C> =
    Box<dyn FnMut(&mut BufferWriter, f64, <C as ContextInfo>::Context<'_>) -> StageActivity + Send>;

impl<C: ContextInfo> Stage<C> {
    pub fn new(
        stage_fn: impl FnMut(&mut BufferWriter, f64, C::Context<'_>) -> StageActivity + Send + 'static,
    ) -> Self {
        Self {
            stage_fn: Box::new(stage_fn),
        }
    }

    pub fn process(
        &mut self,
        buffers: &mut BufferWriter,
        render_window_secs: f64,
        context: C::Context<'_>,
    ) -> StageActivity {
        (self.stage_fn)(buffers, render_window_secs, context)
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
