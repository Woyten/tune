use std::collections::HashMap;

use crate::{
    automation::{AutomatableValue, AutomatedValue, Automation, AutomationSpec, ContextInfo},
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

    pub fn create_value<V: AutomatableValue<C>>(&self, value: V) -> V::Created {
        value.use_creator(self)
    }

    pub fn create_template(&self, template_name: &str) -> Option<C::Created>
    where
        C: AutomationSpec,
    {
        self.templates
            .get(template_name)
            .map(|template| Self::new_without_nesting().create_value(template))
    }

    pub fn create_stage<V: AutomatableValue<C>>(
        &self,
        value: V,
        mut stage_fn: impl FnMut(&mut BufferWriter, <V::Created as AutomatedValue<C>>::Value) -> StageActivity
            + Send
            + 'static,
    ) -> Stage<C>
    where
        V::Created: AutomatedValue<C> + Send + 'static,
    {
        let mut value = self.create_value(value);
        Stage::new(move |buffers, render_window_secs, context| {
            stage_fn(buffers, value.use_context(render_window_secs, context))
        })
    }

    pub fn create_automation<V: AutomatableValue<C>>(
        &self,
        value: V,
        mut automation_fn: impl FnMut(f64, C::Context<'_>, <V::Created as AutomatedValue<C>>::Value) -> f64
            + Send
            + 'static,
    ) -> Automation<C>
    where
        V::Created: AutomatedValue<C> + Send + 'static,
    {
        let mut value = self.create_value(value);
        Automation {
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
