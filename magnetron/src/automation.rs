use std::marker::PhantomData;

use crate::spec::Spec;

type AutomationFn<T> = Box<dyn FnMut(&AutomationContext<T>) -> f64 + Send>;

pub struct Automation<T> {
    pub(crate) automation_fn: AutomationFn<T>,
}

pub struct AutomationContext<'a, T> {
    pub render_window_secs: f64,
    pub payload: &'a T,
}

impl<'a, T> AutomationContext<'a, T> {
    pub fn read<V: AutomatedValue<Context = T>>(&self, value: &mut V) -> V::Value {
        value.use_context(self)
    }
}

impl<T> AutomatedValue for Automation<T> {
    type Context = T;
    type Value = f64;

    fn use_context(&mut self, context: &AutomationContext<Self::Context>) -> Self::Value {
        (self.automation_fn)(context)
    }
}

pub trait AutomatedValue {
    type Context;
    type Value;

    fn use_context(&mut self, context: &AutomationContext<Self::Context>) -> Self::Value;
}

pub type SendablePhantomData<T> = PhantomData<fn(T)>;

impl<T> AutomatedValue for SendablePhantomData<T> {
    type Context = T;
    type Value = ();

    fn use_context(&mut self, _context: &AutomationContext<Self::Context>) -> Self::Value {}
}

impl<A1: AutomatedValue, A2: AutomatedValue<Context = A1::Context>> AutomatedValue for (A1, A2) {
    type Context = A1::Context;
    type Value = (A1::Value, A2::Value);

    fn use_context(&mut self, context: &AutomationContext<Self::Context>) -> Self::Value {
        (context.read(&mut self.0), context.read(&mut self.1))
    }
}

impl<
        A1: AutomatedValue,
        A2: AutomatedValue<Context = A1::Context>,
        A3: AutomatedValue<Context = A1::Context>,
    > AutomatedValue for (A1, A2, A3)
{
    type Context = A1::Context;
    type Value = (A1::Value, A2::Value, A3::Value);

    fn use_context(&mut self, context: &AutomationContext<Self::Context>) -> Self::Value {
        (
            context.read(&mut self.0),
            context.read(&mut self.1),
            context.read(&mut self.2),
        )
    }
}

impl<A: AutomatedValue> AutomatedValue for Option<A> {
    type Context = A::Context;
    type Value = Option<A::Value>;

    fn use_context(&mut self, context: &AutomationContext<Self::Context>) -> Self::Value {
        self.as_mut().map(|value| context.read(value))
    }
}

pub trait AutomationSpec: Spec<Created = Automation<Self::Context>> {
    type Context: 'static;
}
