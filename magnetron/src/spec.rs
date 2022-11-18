use std::collections::HashMap;

use crate::{
    automation::{AutomatedValue, Automation, AutomationContext, AutomationSpec},
    envelope::EnvelopeSpec,
    BufferWriter, Stage, StageState,
};

pub struct Creator<A> {
    envelopes: HashMap<String, EnvelopeSpec<A>>,
}

impl<A> Creator<A> {
    pub fn new(envelopes: HashMap<String, EnvelopeSpec<A>>) -> Self {
        Self { envelopes }
    }

    pub fn create<S: Spec<A>>(&self, spec: S) -> S::Created {
        spec.use_creator(self)
    }

    pub fn create_envelope(&self, envelop_name: &str) -> Option<Stage<A::Context>>
    where
        A: AutomationSpec,
    {
        self.envelopes
            .get(envelop_name)
            .map(|spec| self.create(spec))
    }

    pub fn create_stage<T, S: Spec<A>>(
        &self,
        input: S,
        mut stage_fn: impl FnMut(&mut BufferWriter, <S::Created as AutomatedValue<T>>::Value) -> StageState
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

    pub fn create_automation<T, S: Spec<A>>(
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

pub trait Spec<A> {
    type Created;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created;
}

impl<A> Spec<A> for () {
    type Created = ();

    fn use_creator(&self, _creator: &Creator<A>) -> Self::Created {}
}

impl<A, S: Spec<A>> Spec<A> for &S {
    type Created = S::Created;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        S::use_creator(self, creator)
    }
}

impl<A, S1: Spec<A>, S2: Spec<A>> Spec<A> for (S1, S2) {
    type Created = (S1::Created, S2::Created);

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        (creator.create(&self.0), creator.create(&self.1))
    }
}

impl<A, S1: Spec<A>, S2: Spec<A>, S3: Spec<A>> Spec<A> for (S1, S2, S3) {
    type Created = (S1::Created, S2::Created, S3::Created);

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        (
            creator.create(&self.0),
            creator.create(&self.1),
            creator.create(&self.2),
        )
    }
}

impl<A, S: Spec<A>> Spec<A> for Option<S> {
    type Created = Option<S::Created>;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        self.as_ref().map(|spec| creator.create(spec))
    }
}
