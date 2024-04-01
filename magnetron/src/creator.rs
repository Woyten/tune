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

    pub fn create_automatable<V: Automatable<C>>(&self, value: V) -> V::Created {
        value.use_creator(self)
    }

    pub fn create_template(&self, template_name: &str) -> Option<C::Created>
    where
        C: AutomatableParam,
    {
        self.templates
            .get(template_name)
            .map(|template| Self::new_without_nesting().create_automatable(template))
    }

    pub fn create_stage<V: Automatable<C>>(
        &self,
        value: V,
        mut stage_fn: impl FnMut(&mut BufferWriter, <V::Created as Automated<C>>::Value) -> StageActivity
            + Send
            + 'static,
    ) -> Stage<C>
    where
        V::Created: Automated<C> + Send + 'static,
    {
        let mut value = self.create_automatable(value);
        Stage::new(move |buffers, context| {
            stage_fn(
                buffers,
                value.use_context(buffers.render_window_secs(), context),
            )
        })
    }

    pub fn create_automation<V: Automatable<C>>(
        &self,
        value: V,
        mut automation_fn: impl FnMut(C::Context<'_>, <V::Created as Automated<C>>::Value) -> f64
            + Send
            + 'static,
    ) -> AutomatedValue<C>
    where
        V::Created: Automated<C> + Send + 'static,
    {
        let mut value = self.create_automatable(value);
        AutomatedValue {
            automation_fn: Box::new(move |render_window_secs, context| {
                automation_fn(context, value.use_context(render_window_secs, context))
            }),
        }
    }
}
