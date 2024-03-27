//! Core concepts for using variable values and external parameters in audio processing pipelines.

use crate::creator::Creator;

/// Trait alias defining all types and methods required for driving the generation of an automated value.
///
/// Combines the traits [`ContextInfo`] and [`Automatable`] with [`Automatable`] yielding a variable, context-dependent `f64`.
pub trait AutomatableValue:
    ContextInfo + Automatable<Self, Created = Self::Automated> + Sized
{
    type Automated: Automated<Self, Value = f64> + Send + 'static;
}

impl<A> AutomatableValue for A
where
    A: ContextInfo + Automatable<Self> + Sized,
    A::Created: Automated<Self, Value = f64> + Send + 'static,
{
    type Automated = A::Created;
}

/// Trait encoding type information about the context that is passed to the stages during processing.
///
/// Consumers like [`Stage`](`crate::stage::Stage`) and [`AutomatedValue`] use a type parameter `C` which encodes the type of contextual information they can process. The parameter `C` is not a direct representation of the context type but uses an indirection via [`ContextInfo::Context`]. This indirection is necessary in order to prevent lifetimes from bubbling up into other types.
///
/// # Example
///
/// In this example we want to process a context of type `(&MyContext1, &MyContext2)`. Note that the lifetimes of the references are not repeated in the outer type `MyContextInfo`.
///
/// ```
/// # use magnetron::automation::ContextInfo;
/// struct MyContext1;
/// struct MyContext2;
///
/// struct MyContextInfo;
///
/// impl ContextInfo for MyContextInfo {
///     type Context<'a> = (&'a MyContext1, &'a MyContext2);
/// }
/// ```
pub trait ContextInfo {
    /// The actual type that is passed to the consumers.
    type Context<'a>: Copy;
}

impl ContextInfo for () {
    type Context<'a> = ();
}

/// A nestable factory for materializing variable values that can get automated over a context `C`.
///
/// Chaining and nesting is supported, i.e. if `a`, `b` and `c` are [`Automatable`], then `(a, (b, c))` is [`Automatable`] as well.
pub trait Automatable<C> {
    /// The type of the variable value being materialized.
    type Created;

    /// Materializes the current instance into a variable value.
    fn use_creator(&self, creator: &Creator<C>) -> Self::Created;
}

/// A nestable variable value that can get automated over a context `C`.
///
/// Chaining and nesting is supported, i.e. if `a`, `b` and `c` are [`Automated`], then `(a, (b, c))` is [`Automated`] as well.
pub trait Automated<C: ContextInfo> {
    /// The actual type of the variable value after querying the context.
    type Value;

    /// Queries the context to retrieve a snapshot of the variable value.
    fn use_context(&mut self, render_window_secs: f64, context: C::Context<'_>) -> Self::Value;
}

impl<C> Automatable<C> for () {
    type Created = ();

    fn use_creator(&self, _creator: &Creator<C>) -> Self::Created {}
}

impl<C: ContextInfo> Automated<C> for () {
    type Value = ();

    fn use_context(&mut self, _render_window_secs: f64, _context: C::Context<'_>) -> Self::Value {}
}

impl<C, V: Automatable<C>> Automatable<C> for &V {
    type Created = V::Created;

    fn use_creator(&self, creator: &Creator<C>) -> Self::Created {
        V::use_creator(self, creator)
    }
}

impl<C: ContextInfo, A: Automated<C>> Automated<C> for &mut A {
    type Value = A::Value;

    fn use_context(&mut self, render_window_secs: f64, context: C::Context<'_>) -> Self::Value {
        A::use_context(self, render_window_secs, context)
    }
}

impl<C, V1: Automatable<C>, V2: Automatable<C>> Automatable<C> for (V1, V2) {
    type Created = (V1::Created, V2::Created);

    fn use_creator(&self, creator: &Creator<C>) -> Self::Created {
        (self.0.use_creator(creator), self.1.use_creator(creator))
    }
}

impl<C: ContextInfo, A1: Automated<C>, A2: Automated<C>> Automated<C> for (A1, A2) {
    type Value = (A1::Value, A2::Value);

    fn use_context(&mut self, render_window_secs: f64, context: C::Context<'_>) -> Self::Value {
        (
            self.0.use_context(render_window_secs, context),
            self.1.use_context(render_window_secs, context),
        )
    }
}

impl<C, V1: Automatable<C>, V2: Automatable<C>, V3: Automatable<C>> Automatable<C>
    for (V1, V2, V3)
{
    type Created = (V1::Created, V2::Created, V3::Created);

    fn use_creator(&self, creator: &Creator<C>) -> Self::Created {
        (
            self.0.use_creator(creator),
            self.1.use_creator(creator),
            self.2.use_creator(creator),
        )
    }
}

impl<C: ContextInfo, A1: Automated<C>, A2: Automated<C>, A3: Automated<C>> Automated<C>
    for (A1, A2, A3)
{
    type Value = (A1::Value, A2::Value, A3::Value);

    fn use_context(&mut self, render_window_secs: f64, context: C::Context<'_>) -> Self::Value {
        (
            self.0.use_context(render_window_secs, context),
            self.1.use_context(render_window_secs, context),
            self.2.use_context(render_window_secs, context),
        )
    }
}

impl<C, V: Automatable<C>> Automatable<C> for Option<V> {
    type Created = Option<V::Created>;

    fn use_creator(&self, creator: &Creator<C>) -> Self::Created {
        self.as_ref().map(|spec| spec.use_creator(creator))
    }
}

impl<C: ContextInfo, A: Automated<C>> Automated<C> for Option<A> {
    type Value = Option<A::Value>;

    fn use_context(&mut self, render_window_secs: f64, context: C::Context<'_>) -> Self::Value {
        self.as_mut()
            .map(|value| value.use_context(render_window_secs, context))
    }
}

/// A concrete implementation of [`Automated`] yielding a variable, context-dependent `f64`.
///
/// This type is used to retrieve the actual numerical values required in the implementation bodies of [`Stage`](`crate::stage::Stage`)s or other (nested) [`AutomatedValue`]s.
///
/// Use [`Creator::create_automation`] to create a new [`AutomatedValue`].
pub struct AutomatedValue<C: ContextInfo> {
    pub(crate) automation_fn: AutomationFn<C>,
}

type AutomationFn<C> = Box<dyn FnMut(f64, <C as ContextInfo>::Context<'_>) -> f64 + Send>;

impl<C: ContextInfo> Automated<C> for AutomatedValue<C> {
    type Value = f64;

    fn use_context(&mut self, render_window_secs: f64, context: C::Context<'_>) -> Self::Value {
        (self.automation_fn)(render_window_secs, context)
    }
}
