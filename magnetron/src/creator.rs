use std::collections::HashMap;

use crate::{
    automation::{AutomatableValue, AutomatedValue, Automation, AutomationContext, AutomationSpec},
    buffer::BufferWriter,
    Stage, StageState,
};

pub struct Creator<A> {
    templates: HashMap<String, A>,
}

impl<A: AutomationSpec> Creator<A> {
    pub fn new(templates: HashMap<String, A>) -> Self {
        Self { templates }
    }

    fn new_without_nesting() -> Self {
        Self::new(HashMap::new())
    }

    pub fn create_value<V: AutomatableValue<A>>(&self, value: V) -> V::Created {
        value.use_creator(self)
    }

    pub fn create_template(&self, template_name: &str) -> Option<A::AutomatedValue> {
        self.templates
            .get(template_name)
            .map(|template| Self::new_without_nesting().create_value(template))
    }

    pub fn create_stage<V: AutomatableValue<A>>(
        &self,
        value: V,
        mut stage_fn: impl FnMut(
                &mut BufferWriter,
                <V::Created as AutomatedValue<A::Context>>::Value,
            ) -> StageState
            + Send
            + 'static,
    ) -> Stage<A::Context> {
        let mut value = self.create_value(value);
        Stage {
            state: StageState::Active,
            stage_fn: Box::new(move |buffers, context| stage_fn(buffers, context.read(&mut value))),
        }
    }

    pub fn create_automation<V: AutomatableValue<A>>(
        &self,
        value: V,
        mut automation_fn: impl FnMut(
                &AutomationContext<A::Context>,
                <V::Created as AutomatedValue<A::Context>>::Value,
            ) -> f64
            + Send
            + 'static,
    ) -> Automation<A::Context> {
        let mut value = self.create_value(value);
        Automation {
            automation_fn: Box::new(move |context| {
                automation_fn(context, context.read(&mut value))
            }),
        }
    }
}
