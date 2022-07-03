use std::marker::PhantomData;

use crate::{spec::Spec, waveform::WaveformState};

pub struct Automation<S> {
    pub(crate) automation_fn: Box<dyn FnMut(&AutomationContext<S>) -> f64 + Send>,
}

pub struct AutomationContext<'a, S> {
    pub render_window_secs: f64,
    pub state: &'a WaveformState,
    pub storage: &'a S,
}

impl<'a, S> AutomationContext<'a, S> {
    pub fn read<V: AutomatedValue<Storage = S>>(&self, value: &mut V) -> V::Value {
        value.use_context(self)
    }
}

impl<S> AutomatedValue for Automation<S> {
    type Storage = S;
    type Value = f64;

    fn use_context(&mut self, context: &AutomationContext<Self::Storage>) -> Self::Value {
        (self.automation_fn)(context)
    }
}

pub trait AutomatedValue {
    type Storage;
    type Value;

    fn use_context(&mut self, context: &AutomationContext<Self::Storage>) -> Self::Value;
}

impl<A: AutomatedValue> AutomatedValue for PhantomData<A> {
    type Storage = A::Storage;
    type Value = ();

    fn use_context(&mut self, _context: &AutomationContext<Self::Storage>) -> Self::Value {}
}

impl<A1: AutomatedValue, A2: AutomatedValue<Storage = A1::Storage>> AutomatedValue for (A1, A2) {
    type Storage = A1::Storage;
    type Value = (A1::Value, A2::Value);

    fn use_context(&mut self, context: &AutomationContext<Self::Storage>) -> Self::Value {
        (context.read(&mut self.0), context.read(&mut self.1))
    }
}

impl<
        A1: AutomatedValue,
        A2: AutomatedValue<Storage = A1::Storage>,
        A3: AutomatedValue<Storage = A1::Storage>,
    > AutomatedValue for (A1, A2, A3)
{
    type Storage = A1::Storage;
    type Value = (A1::Value, A2::Value, A3::Value);

    fn use_context(&mut self, context: &AutomationContext<Self::Storage>) -> Self::Value {
        (
            context.read(&mut self.0),
            context.read(&mut self.1),
            context.read(&mut self.2),
        )
    }
}

impl<A: AutomatedValue> AutomatedValue for Option<A> {
    type Storage = A::Storage;
    type Value = Option<A::Value>;

    fn use_context(&mut self, context: &AutomationContext<Self::Storage>) -> Self::Value {
        self.as_mut().map(|value| context.read(value))
    }
}

pub trait AutomationSpec: Spec<Created = Automation<Self::Storage>> {
    type Storage: 'static;
}
