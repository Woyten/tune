//! Core concepts for using time-dependent data and external parameters in audio processing pipelines.

use crate::{
    buffer::BufferWriter,
    stage::{Stage, StageActivity},
};

/// Factory for creating [`Stage`]s and [`AutomatedValue`]s.
#[derive(Clone, Debug)]
pub struct AutomationFactory<C: CreationInfo> {
    context: C::Context,
}

impl<C: CreationInfo> AutomationFactory<C> {
    /// Creates a new [`AutomationFactory`] with the given context.
    pub fn new(context: C::Context) -> Self {
        Self { context }
    }

    /// Returns a reference to the inner context.
    pub fn context(&self) -> &C::Context {
        &self.context
    }

    /// Returns a mutable reference to the inner context.
    pub fn context_mut(&mut self) -> &mut C::Context {
        &mut self.context
    }

    /// Materializes the given `automatable` into a variable data type.
    pub fn automate<T: Automatable<C>>(&mut self, automatable: T) -> T::Output {
        automatable.create(self)
    }
}

/// Self-describing [`Automatable`] defining all types and methods required for retrieving automated parameters.
///
/// Combines the traits [`CreationInfo`], [`QueryInfo`] and [`Automatable`] with [`Automatable`] yielding a time-dependent and context-dependent `f64`.
pub trait AutomatableParam:
    CreationInfo + QueryInfo + Automatable<Self, Output = Self::Automated> + Sized
{
    type Automated: for<'a> Automated<Self, Output<'a> = f64> + Send + 'static;
}

impl<T> AutomatableParam for T
where
    T: CreationInfo + QueryInfo + Automatable<Self> + Sized,
    T::Output: for<'a> Automated<Self, Output<'a> = f64> + Send + 'static,
{
    type Automated = T::Output;
}

/// Trait encoding type information about the context that is passed to the [`Stage`]s during creation.
pub trait CreationInfo {
    type Context;
}

impl CreationInfo for () {
    type Context = ();
}

/// Trait encoding type information about the context that is passed to the [`Stage`]s during processing.
///
/// Consumers like [`Stage`] and [`AutomatedValue`] use a type parameter `A` which encodes the type of contextual information they can process. The parameter `A` is not a direct representation of the context type but uses an indirection via [`QueryInfo::Context`]. This indirection is helpful in order to prevent lifetimes from bubbling up into other types.
///
/// # Examples
///
/// In this example we want to process a context of type `(&MyContext1, &MyContext2)`. Note that the lifetimes of the references are not repeated in the outer type `MyQueryInfo`.
///
/// ```
/// # use magnetron::automation::QueryInfo;
/// struct MyContext1;
/// struct MyContext2;
///
/// struct MyQueryInfo;
///
/// impl QueryInfo for MyQueryInfo {
///     type Context<'a> = (&'a MyContext1, &'a MyContext2);
/// }
/// ```
pub trait QueryInfo {
    /// The actual type that is passed to the consumers.
    type Context<'a>: Copy;
}

impl QueryInfo for () {
    type Context<'a> = ();
}

/// A nestable factory for materializing variable data types that can get automated over a context `A`.
///
/// Chaining and nesting is supported, i.e. if `a`, `b` and `c` are [`Automatable`], then `(a, (b, c))` is [`Automatable`] as well.
pub trait Automatable<C: CreationInfo> {
    /// The variable data type to be created.
    type Output;

    /// Materializes the current instance into a variable data type.
    fn create(&self, factory: &mut AutomationFactory<C>) -> Self::Output;
}

/// A nestable variable data type that can get automated over a context `A`.
///
/// Chaining and nesting is supported, i.e. if `a`, `b` and `c` are [`Automated`], then `(a, (b, c))` is [`Automated`] as well.
pub trait Automated<Q: QueryInfo> {
    /// The actual type of the variable data after querying the context.
    type Output<'a>
    where
        Self: 'a;

    /// Queries the context to retrieve a snapshot of the variable data.
    fn query(&mut self, render_window_secs: f64, context: Q::Context<'_>) -> Self::Output<'_>;

    /// Creates an [`AutomatedValue`] from the given `automation_fn`.
    ///
    /// When [`AutomatedValue::query`] is invoked on the return value, `automation_fn` is invoked with the most recent evaluation of `self`.
    fn into_automation(
        mut self,
        mut automation_fn: impl FnMut(Q::Context<'_>, Self::Output<'_>) -> f64 + Send + 'static,
    ) -> AutomatedValue<Q>
    where
        Self: Send + Sized + 'static,
    {
        AutomatedValue {
            automation_fn: Box::new(move |render_window_secs, context| {
                automation_fn(context, self.query(render_window_secs, context))
            }),
        }
    }

    /// Creates a [`Stage`] from the given `stage_fn`.
    ///
    /// When [`Stage::process`] is invoked on the return value, `stage_fn` is invoked with the most recent evaluation of `self`.
    fn into_stage(
        mut self,
        mut stage_fn: impl FnMut(&mut BufferWriter, Self::Output<'_>) -> StageActivity + Send + 'static,
    ) -> Stage<Q>
    where
        Self: Send + Sized + 'static,
    {
        Stage::new(move |buffers, context| {
            stage_fn(buffers, self.query(buffers.render_window_secs(), context))
        })
    }
}

/// An [`Automatable`] yielding the width of the window being rendered in seconds.
pub struct RenderWindowSecs;

impl<C: CreationInfo> Automatable<C> for RenderWindowSecs {
    type Output = RenderWindowSecs;

    fn create(&self, _factory: &mut AutomationFactory<C>) -> Self::Output {
        RenderWindowSecs
    }
}

impl<Q: QueryInfo> Automated<Q> for RenderWindowSecs {
    type Output<'a> = f64;

    fn query(&mut self, render_window_secs: f64, _context: Q::Context<'_>) -> Self::Output<'_> {
        render_window_secs
    }
}

impl<C: CreationInfo> Automatable<C> for () {
    type Output = ();

    fn create(&self, _factory: &mut AutomationFactory<C>) -> Self::Output {}
}

impl<Q: QueryInfo> Automated<Q> for () {
    type Output<'a> = ();

    fn query(&mut self, _render_window_secs: f64, _context: Q::Context<'_>) -> Self::Output<'_> {}
}

impl<C: CreationInfo, T> Automatable<C> for &T
where
    T: Automatable<C>,
{
    type Output = T::Output;

    fn create(&self, factory: &mut AutomationFactory<C>) -> Self::Output {
        T::create(self, factory)
    }
}

impl<Q: QueryInfo, T> Automated<Q> for &mut T
where
    T: Automated<Q>,
{
    type Output<'a> = T::Output<'a> where Self: 'a;

    fn query(&mut self, render_window_secs: f64, context: Q::Context<'_>) -> Self::Output<'_> {
        T::query(self, render_window_secs, context)
    }
}

macro_rules! impl_automatable_for_tuple {
    ($($param:ident),*) => {
        impl<C: CreationInfo, $($param),*> Automatable<C> for ($($param),+)
        where $($param: Automatable<C>),*
        {
            type Output = ($($param::Output),+);

            #[allow(non_snake_case)]
            fn create(&self, factory: &mut AutomationFactory<C>) -> Self::Output {
                let ($($param),*) = self;
                (($($param.create(factory)),*))
            }
        }
    };
}

impl_automatable_for_tuple!(T1, T2);
impl_automatable_for_tuple!(T1, T2, T3);
impl_automatable_for_tuple!(T1, T2, T3, T4);
impl_automatable_for_tuple!(T1, T2, T3, T4, T5);

macro_rules! impl_automated_for_tuple {
    ($($param:ident),*) => {
        impl<Q: QueryInfo, $($param),*> Automated<Q> for ($($param),+)
        where $($param: Automated<Q>),*
        {
            type Output<'a> = ($($param::Output<'a>),+) where Self: 'a;

            #[allow(non_snake_case)]
            fn query(&mut self, render_window_secs: f64, context: Q::Context<'_>) -> Self::Output<'_> {
                let ($($param),*) = self;
                (($($param.query(render_window_secs, context)),*))
            }
        }
    };
}

impl_automated_for_tuple!(T1, T2);
impl_automated_for_tuple!(T1, T2, T3);
impl_automated_for_tuple!(T1, T2, T3, T4);
impl_automated_for_tuple!(T1, T2, T3, T4, T5);

impl<C: CreationInfo, T> Automatable<C> for Option<T>
where
    T: Automatable<C>,
{
    type Output = Option<T::Output>;

    fn create(&self, factory: &mut AutomationFactory<C>) -> Self::Output {
        self.as_ref().map(|spec| spec.create(factory))
    }
}

impl<Q: QueryInfo, T> Automated<Q> for Option<T>
where
    T: Automated<Q>,
{
    type Output<'a> = Option<T::Output<'a>> where Self: 'a;

    fn query(&mut self, render_window_secs: f64, context: Q::Context<'_>) -> Self::Output<'_> {
        self.as_mut()
            .map(|value| value.query(render_window_secs, context))
    }
}

/// A concrete implementation of [`Automated`] yielding a time-dependent and context-dependent `f64`.
///
/// This type is used to retrieve the actual numerical values required in the implementation bodies of [`Stage`]s or other (nested) [`AutomatedValue`]s.
///
/// Use [`Automated::into_automation`] to create a new [`AutomatedValue`].
pub struct AutomatedValue<Q: QueryInfo> {
    pub(crate) automation_fn: AutomationFn<Q>,
}

type AutomationFn<Q> = Box<dyn FnMut(f64, <Q as QueryInfo>::Context<'_>) -> f64 + Send>;

impl<Q: QueryInfo> Automated<Q> for AutomatedValue<Q> {
    type Output<'a> = f64 where Self: 'a;

    fn query(&mut self, render_window_secs: f64, context: Q::Context<'_>) -> Self::Output<'_> {
        (self.automation_fn)(render_window_secs, context)
    }
}
