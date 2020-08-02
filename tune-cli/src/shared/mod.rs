//! Code to be shared with other CLIs. At the moment, this module is not intended to become stable API.

use std::{fs::File, path::PathBuf};
use tune::scala::{Scl, SclImportError};

pub fn import_scl_file(file_name: PathBuf) -> Result<Scl, String> {
    File::open(file_name)
        .map_err(|io_err| format!("Could not read scl file: {}", io_err))
        .and_then(|file| {
            Scl::import(file).map_err(|err| match err {
                SclImportError::IoError(err) => format!("Could not read scl file: {}", err),
                SclImportError::ParseError { line_number, kind } => format!(
                    "Could not parse scl file at line {} ({:?})",
                    line_number, kind
                ),
                SclImportError::StructuralError(err) => format!("Malformed scl file ({:?})", err),
                SclImportError::BuildError(err) => format!("Unsupported scl file ({:?})", err),
            })
        })
}
