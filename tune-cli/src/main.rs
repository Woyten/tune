use std::{env, io::ErrorKind};
use tune_cli::{self, CliError, CliResult};

fn main() -> CliResult<()> {
    match tune_cli::run_in_shell_env(env::args()) {
        // The BrokenPipe case occurs when stdout tries to communicate with a process that has already terminated.
        // Since tune is an idempotent tool with repeatable results, it is okay to ignore this error and terminate successfully.
        Err(CliError::IoError(err)) if err.kind() == ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}
