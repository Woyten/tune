use std::fmt::Debug;
use std::fmt::Display;

pub trait ResultExt<T, E>: Sized {
    fn into_result(self) -> Result<T, E>;

    fn debug_err<F>(self, context: &str) -> Result<T, F>
    where
        E: Debug,
        F: From<String>,
    {
        self.into_result()
            .map_err(|err| F::from(format!("{context}\n{err:#?}")))
    }

    fn display_err<F>(self, context: &str) -> Result<T, F>
    where
        E: Display,
        F: From<String>,
    {
        self.into_result()
            .map_err(|err| F::from(format!("{context}\n{err}")))
    }
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn into_result(self) -> Result<T, E> {
        self
    }
}
