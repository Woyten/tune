use std::collections::HashMap;

use crate::{
    automation::{AutomatableValue, AutomatedValue, Automation, AutomationContext, AutomationSpec},
    envelope::EnvelopeSpec,
    BufferWriter, Stage, StageState,
};

pub struct Creator<A> {
    templates: HashMap<String, A>,
    envelopes: HashMap<String, EnvelopeSpec<A>>,
}

impl<A: AutomationSpec> Creator<A> {
    pub fn new(templates: HashMap<String, A>, envelopes: HashMap<String, EnvelopeSpec<A>>) -> Self {
        Self {
            templates,
            envelopes,
        }
    }

    fn new_without_nesting() -> Creator<A> {
        Self::new(HashMap::new(), HashMap::new())
    }

    pub fn create<V: AutomatableValue<A>>(&self, value: V) -> V::Created {
        value.use_creator(self)
    }

    pub fn create_template(&self, template_name: &str) -> Option<A::AutomatedValue> {
        self.templates
            .get(template_name)
            .map(|spec| Self::new_without_nesting().create(spec))
    }

    pub fn create_envelope(&self, envelope_name: &str) -> Option<Stage<A::Context>> {
        self.envelopes
            .get(envelope_name)
            .map(|spec| spec.use_creator(self))
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
        let mut value = self.create(value);
        Stage {
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
        let mut value = self.create(value);
        Automation {
            automation_fn: Box::new(move |context| {
                automation_fn(context, context.read(&mut value))
            }),
        }
    }
}
