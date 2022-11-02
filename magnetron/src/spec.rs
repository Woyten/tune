use std::collections::HashMap;

use crate::{
    automation::{AutomatedValue, Automation, AutomationContext},
    waveform::{Envelope, Stage},
    BufferWriter,
};

pub struct Creator {
    envelope_map: HashMap<String, Envelope>,
}

impl Creator {
    pub fn new(envelope_map: HashMap<String, Envelope>) -> Self {
        Self { envelope_map }
    }

    pub fn create<S: Spec>(&self, spec: S) -> S::Created {
        spec.use_creator(self)
    }

    pub fn create_envelope(&self, envelop_name: &str) -> Option<Envelope> {
        self.envelope_map.get(envelop_name).cloned()
    }

    pub fn create_stage<T, S: Spec>(
        &self,
        input: S,
        mut stage_fn: impl FnMut(&mut BufferWriter, <S::Created as AutomatedValue<T>>::Value)
            + Send
            + 'static,
    ) -> Stage<T>
    where
        S::Created: AutomatedValue<T> + Send + 'static,
    {
        let mut input = self.create(input);
        Stage {
            stage_fn: Box::new(move |buffers, context| stage_fn(buffers, context.read(&mut input))),
        }
    }

    pub fn create_automation<T, S: Spec>(
        &self,
        input: S,
        mut automation_fn: impl FnMut(&AutomationContext<T>, <S::Created as AutomatedValue<T>>::Value) -> f64
            + Send
            + 'static,
    ) -> Automation<T>
    where
        S::Created: AutomatedValue<T> + Send + 'static,
    {
        let mut input = self.create(input);
        Automation {
            automation_fn: Box::new(move |context| {
                automation_fn(context, context.read(&mut input))
            }),
        }
    }
}

pub trait Spec {
    type Created;

    fn use_creator(&self, creator: &Creator) -> Self::Created;
}

impl Spec for () {
    type Created = ();

    fn use_creator(&self, _creator: &Creator) -> Self::Created {}
}

impl<S: Spec> Spec for &S {
    type Created = S::Created;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        S::use_creator(self, creator)
    }
}

impl<S1: Spec, S2: Spec> Spec for (S1, S2) {
    type Created = (S1::Created, S2::Created);

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        (creator.create(&self.0), creator.create(&self.1))
    }
}

impl<S1: Spec, S2: Spec, S3: Spec> Spec for (S1, S2, S3) {
    type Created = (S1::Created, S2::Created, S3::Created);

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        (
            creator.create(&self.0),
            creator.create(&self.1),
            creator.create(&self.2),
        )
    }
}

impl<S: Spec> Spec for Option<S> {
    type Created = Option<S::Created>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        self.as_ref().map(|spec| creator.create(spec))
    }
}
