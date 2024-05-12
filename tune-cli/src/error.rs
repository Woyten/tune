use std::fmt::Debug;

pub trait ResultExt<T, E> {
    fn handle_error<F>(self, context: &str) -> Result<T, F>
    where
        E: Debug,
        F: From<String>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn handle_error<F>(self, context: &str) -> Result<T, F>
    where
        E: Debug,
        F: From<String>,
    {
        self.map_err(|err| F::from(format!("{context}\n{err:#?}")))
    }
}
