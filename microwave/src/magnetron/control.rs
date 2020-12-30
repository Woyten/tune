pub trait Controller: Clone + Send + 'static {
    type Storage;

    fn read(&self, storage: &Self::Storage) -> f64;
}

#[derive(Clone)]
pub enum NoControl {}

impl Controller for NoControl {
    type Storage = ();

    fn read(&self, _storage: &Self::Storage) -> f64 {
        unreachable!("NoControl is inhabitable")
    }
}
