use crate::creator::Creator;

pub trait AutomationSpec: AutomatableValue<Self, Created = Self::AutomatedValue> + Sized {
    type Context;
    type AutomatedValue: AutomatedValue<Self::Context, Value = f64> + Send + 'static;
}

pub trait AutomatableValue<A: AutomationSpec> {
    type Created: AutomatedValue<A::Context> + Send + 'static;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created;
}

pub trait AutomatedValue<T> {
    type Value;

    fn use_context(&mut self, context: &AutomationContext<T>) -> Self::Value;
}

pub struct AutomationContext<'a, T> {
    pub render_window_secs: f64,
    pub payload: &'a T,
}

impl<'a, T> AutomationContext<'a, T> {
    pub fn read<V: AutomatedValue<T>>(&self, value: &mut V) -> V::Value {
        value.use_context(self)
    }
}

impl<A: AutomationSpec> AutomatableValue<A> for () {
    type Created = ();

    fn use_creator(&self, _creator: &Creator<A>) -> Self::Created {}
}

impl<T> AutomatedValue<T> for () {
    type Value = ();

    fn use_context(&mut self, _context: &AutomationContext<T>) -> Self::Value {}
}

impl<A: AutomationSpec, V: AutomatableValue<A>> AutomatableValue<A> for &V {
    type Created = V::Created;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        V::use_creator(self, creator)
    }
}

impl<T, A: AutomatedValue<T>> AutomatedValue<T> for &mut A {
    type Value = A::Value;

    fn use_context(&mut self, context: &AutomationContext<T>) -> Self::Value {
        A::use_context(self, context)
    }
}

impl<A: AutomationSpec, V1: AutomatableValue<A>, V2: AutomatableValue<A>> AutomatableValue<A>
    for (V1, V2)
{
    type Created = (V1::Created, V2::Created);

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        (creator.create_value(&self.0), creator.create_value(&self.1))
    }
}

impl<T, A1: AutomatedValue<T>, A2: AutomatedValue<T>> AutomatedValue<T> for (A1, A2) {
    type Value = (A1::Value, A2::Value);

    fn use_context(&mut self, context: &AutomationContext<T>) -> Self::Value {
        (context.read(&mut self.0), context.read(&mut self.1))
    }
}

impl<
        A: AutomationSpec,
        V1: AutomatableValue<A>,
        V2: AutomatableValue<A>,
        V3: AutomatableValue<A>,
    > AutomatableValue<A> for (V1, V2, V3)
{
    type Created = (V1::Created, V2::Created, V3::Created);

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        (
            creator.create_value(&self.0),
            creator.create_value(&self.1),
            creator.create_value(&self.2),
        )
    }
}

impl<T, A1: AutomatedValue<T>, A2: AutomatedValue<T>, A3: AutomatedValue<T>> AutomatedValue<T>
    for (A1, A2, A3)
{
    type Value = (A1::Value, A2::Value, A3::Value);

    fn use_context(&mut self, context: &AutomationContext<T>) -> Self::Value {
        (
            context.read(&mut self.0),
            context.read(&mut self.1),
            context.read(&mut self.2),
        )
    }
}

impl<A: AutomationSpec, V: AutomatableValue<A>> AutomatableValue<A> for Option<V> {
    type Created = Option<V::Created>;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        self.as_ref().map(|spec| creator.create_value(spec))
    }
}

impl<T, A: AutomatedValue<T>> AutomatedValue<T> for Option<A> {
    type Value = Option<A::Value>;

    fn use_context(&mut self, context: &AutomationContext<T>) -> Self::Value {
        self.as_mut().map(|value| context.read(value))
    }
}

pub struct Automation<T> {
    pub(crate) automation_fn: AutomationFn<T>,
}

type AutomationFn<T> = Box<dyn FnMut(&AutomationContext<T>) -> f64 + Send>;

impl<T> AutomatedValue<T> for Automation<T> {
    type Value = f64;

    fn use_context(&mut self, context: &AutomationContext<T>) -> Self::Value {
        (self.automation_fn)(context)
    }
}
