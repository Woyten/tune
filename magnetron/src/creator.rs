use crate::{
    automation::{Automatable, Automated, AutomatedValue, AutomationInfo, CreationInfo},
    buffer::BufferWriter,
    stage::{Stage, StageActivity},
};

pub struct Creator<C: CreationInfo> {
    context: C::CreationContext,
}

impl<C: CreationInfo> Creator<C> {
    pub fn new(context: C::CreationContext) -> Self {
        Self { context }
    }

    pub fn context(&self) -> &C::CreationContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut C::CreationContext {
        &mut self.context
    }

    pub fn create_automatable<V: Automatable<C>>(&self, value: V) -> V::Created {
        value.use_creator(self)
    }

    pub fn create_stage<A, V>(
        &self,
        value: V,
        mut stage_fn: impl FnMut(&mut BufferWriter, <V::Created as Automated<A>>::Value) -> StageActivity
            + Send
            + 'static,
    ) -> Stage<A>
    where
        A: AutomationInfo,
        V: Automatable<C>,
        V::Created: Automated<A> + Send + 'static,
    {
        let mut value = self.create_automatable(value);
        Stage::new(move |buffers, render_window_secs, context| {
            stage_fn(buffers, value.use_context(render_window_secs, context))
        })
    }

    pub fn create_automation<A, V>(
        &self,
        value: V,
        mut automation_fn: impl FnMut(f64, A::AutomationContext<'_>, <V::Created as Automated<A>>::Value) -> f64
            + Send
            + 'static,
    ) -> AutomatedValue<A>
    where
        A: AutomationInfo,
        V: Automatable<C>,
        V::Created: Automated<A> + Send + 'static,
    {
        let mut value = self.create_automatable(value);
        AutomatedValue {
            automation_fn: Box::new(move |render_window_secs, context| {
                automation_fn(
                    render_window_secs,
                    context,
                    value.use_context(render_window_secs, context),
                )
            }),
        }
    }
}
