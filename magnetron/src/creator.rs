use crate::{
    automation::{Automatable, Automated, AutomatedValue, AutomationInfo, CreationInfo},
    buffer::BufferWriter,
    stage::{Stage, StageActivity},
};

#[derive(Clone, Debug)]
pub struct Creator<C: CreationInfo> {
    context: C::Context,
}

impl<C: CreationInfo> Creator<C> {
    pub fn new(context: C::Context) -> Self {
        Self { context }
    }

    pub fn context(&self) -> &C::Context {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut C::Context {
        &mut self.context
    }

    pub fn create<T: Automatable<C>>(&mut self, automatable: T) -> T::Output {
        automatable.use_creator(self)
    }

    pub fn create_stage<T, A>(
        &mut self,
        automatable: T,
        mut stage_fn: impl FnMut(&mut BufferWriter, <T::Output as Automated<A>>::Output) -> StageActivity
            + Send
            + 'static,
    ) -> Stage<A>
    where
        T: Automatable<C>,
        T::Output: Automated<A> + Send + 'static,
        A: AutomationInfo,
    {
        let mut value = self.create(automatable);
        Stage::new(move |buffers, context| {
            stage_fn(
                buffers,
                value.use_context(buffers.render_window_secs(), context),
            )
        })
    }

    pub fn create_automation<T, A>(
        &mut self,
        automatable: T,
        mut automation_fn: impl FnMut(A::Context<'_>, <T::Output as Automated<A>>::Output) -> f64
            + Send
            + 'static,
    ) -> AutomatedValue<A>
    where
        T: Automatable<C>,
        T::Output: Automated<A> + Send + 'static,
        A: AutomationInfo,
    {
        let mut value = self.create(automatable);
        AutomatedValue {
            automation_fn: Box::new(move |render_window_secs, context| {
                automation_fn(context, value.use_context(render_window_secs, context))
            }),
        }
    }
}
