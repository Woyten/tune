//! Code to be shared with other CLIs. At the moment, this module is not intended to become a stable API.

pub mod error {
    pub use crate::error::*;
}
pub mod midi {
    pub use crate::midi::*;
}
pub mod portable {
    pub use crate::portable::*;
}
pub mod scala {
    pub use crate::scala::*;
}
