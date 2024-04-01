use std::collections::HashMap;

use crate::{
    automation::{Automatable, AutomatableParam, Automated, AutomatedValue, ContextInfo},
    buffer::BufferWriter,
    stage::{Stage, StageActivity},
};

pub struct Creator<C> {
    templates: HashMap<String, C>,
}

impl<C: ContextInfo> Creator<C> {
    pub fn new(templates: HashMap<String, C>) -> Self {
        Self { templates }
    }

    fn new_without_nesting() -> Self {
        Self::new(HashMap::new())
    }

    pub fn create<T: Automatable<C>>(&self, automatable: T) -> T::Output {
        automatable.use_creator(self)
    }

    pub fn create_template(&self, template_name: &str) -> Option<C::Output>
    where
        C: AutomatableParam,
    {
        self.templates
            .get(template_name)
            .map(|template| Self::new_without_nesting().create(template))
    }

    pub fn create_stage<T>(
        &self,
        automatable: T,
        mut stage_fn: impl FnMut(&mut BufferWriter, <T::Output as Automated<C>>::Output) -> StageActivity
            + Send
            + 'static,
    ) -> Stage<C>
    where
        T: Automatable<C>,
        T::Output: Automated<C> + Send + 'static,
    {
        let mut value = self.create(automatable);
        Stage::new(move |buffers, context| {
            stage_fn(
                buffers,
                value.use_context(buffers.render_window_secs(), context),
            )
        })
    }

    pub fn create_automation<T>(
        &self,
        automatable: T,
        mut automation_fn: impl FnMut(C::Context<'_>, <T::Output as Automated<C>>::Output) -> f64
            + Send
            + 'static,
    ) -> AutomatedValue<C>
    where
        T: Automatable<C>,
        T::Output: Automated<C> + Send + 'static,
    {
        let mut value = self.create(automatable);
        AutomatedValue {
            automation_fn: Box::new(move |render_window_secs, context| {
                automation_fn(context, value.use_context(render_window_secs, context))
            }),
        }
    }
}
