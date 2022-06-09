use super::{AutomatedValue, AutomationContext};

pub trait Controller: AutomatedValue<Value = f64> + Clone + Send + 'static {}

#[derive(Clone)]
pub enum NoControl {}

impl AutomatedValue for NoControl {
    type Value = f64;
    type Storage = ();

    fn use_context(&mut self, _context: &AutomationContext<Self::Storage>) -> Self::Value {
        unreachable!("NoControl is inhabitable")
    }
}

impl Controller for NoControl {}
