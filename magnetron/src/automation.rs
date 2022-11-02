type AutomationFn<T> = Box<dyn FnMut(&AutomationContext<T>) -> f64 + Send>;

pub struct Automation<T> {
    pub(crate) automation_fn: AutomationFn<T>,
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

impl<T> AutomatedValue<T> for Automation<T> {
    type Value = f64;

    fn use_context(&mut self, context: &AutomationContext<T>) -> Self::Value {
        (self.automation_fn)(context)
    }
}

pub trait AutomatedValue<T> {
    type Value;

    fn use_context(&mut self, context: &AutomationContext<T>) -> Self::Value;
}

impl<T> AutomatedValue<T> for () {
    type Value = ();

    fn use_context(&mut self, _context: &AutomationContext<T>) -> Self::Value {}
}

impl<T, A1: AutomatedValue<T>, A2: AutomatedValue<T>> AutomatedValue<T> for (A1, A2) {
    type Value = (A1::Value, A2::Value);

    fn use_context(&mut self, context: &AutomationContext<T>) -> Self::Value {
        (context.read(&mut self.0), context.read(&mut self.1))
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

impl<T, A: AutomatedValue<T>> AutomatedValue<T> for Option<A> {
    type Value = Option<A::Value>;

    fn use_context(&mut self, context: &AutomationContext<T>) -> Self::Value {
        self.as_mut().map(|value| context.read(value))
    }
}
