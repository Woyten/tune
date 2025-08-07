mod dto;
mod error;
mod est;
mod live;
mod midi;
mod mos;
mod mts;
mod portable;
mod scala;
mod scale;

use std::fmt;
use std::fmt::Display;
use std::fs::File;
use std::io;
use std::io::ErrorKind;
use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use error::ResultExt;
use est::EstOptions;
use futures::executor;
use io::Read;
use live::LiveOptions;
use mos::MosCommand;
use mts::MtsOptions;
use scala::KbmCommand;
use scala::SclOptions;
use scale::DiffOptions;
use scale::DumpOptions;
use scale::ScaleCommand;

#[doc(hidden)]
pub mod shared;

#[derive(Parser)]
#[command(version)]
struct MainOptions {
    /// Write output to a file instead of stdout
    #[arg(long = "of")]
    output_file: Option<PathBuf>,

    #[command(subcommand)]
    command: MainCommand,
}

#[derive(Parser)]
enum MainCommand {
    /// Create a scale file
    #[command(name = "scl")]
    Scl(SclOptions),

    /// Create a keyboard mapping file
    #[command(subcommand, name = "kbm")]
    Kbm(KbmCommand),

    /// Analyze equal-step tunings
    #[command(name = "est")]
    Est(EstOptions),

    /// Find MOS scales from generators or vice versa
    #[command(subcommand, name = "mos")]
    Mos(MosCommand),

    /// Print a scale to stdout
    #[command(subcommand, name = "scale")]
    Scale(ScaleCommand),

    /// Display details of a scale
    #[command(name = "dump")]
    Dump(DumpOptions),

    /// Display differences between a source scale and a target scale
    #[command(name = "diff")]
    Diff(DiffOptions),

    /// Print MIDI Tuning Standard messages and/or send them to MIDI devices
    #[command(name = "mts")]
    Mts(MtsOptions),

    /// Enable synthesizers with limited tuning support to be played in any tuning.
    /// This is achieved by reading MIDI data from a sequencer/keyboard and sending modified MIDI data to a synthesizer.
    /// The sequencer/keyboard and synthesizer can be the same device. In this case, remember to disable local keyboard playback.
    #[command(name = "live")]
    Live(LiveOptions),

    /// List MIDI devices
    #[command(name = "devices")]
    Devices,
}

impl MainOptions {
    async fn run(self) -> Result<(), CliError> {
        let output: Box<dyn Write> = match self.output_file {
            Some(output_file) => Box::new(File::create(output_file)?),
            None => Box::new(io::stdout()),
        };

        let mut app = App {
            input: Box::new(io::stdin()),
            output,
            error: Box::new(io::stderr()),
        };

        self.command.run(&mut app).await
    }
}

impl MainCommand {
    async fn run(self, app: &mut App<'_>) -> CliResult {
        match self {
            MainCommand::Scl(options) => options.run(app),
            MainCommand::Kbm(options) => options.run(app),
            MainCommand::Est(options) => options.run(app),
            MainCommand::Mos(options) => options.run(app),
            MainCommand::Scale(options) => options.run(app),
            MainCommand::Dump(options) => options.run(app),
            MainCommand::Diff(options) => options.run(app),
            MainCommand::Mts(options) => options.run(app),
            MainCommand::Live(options) => options.run(app).await,
            MainCommand::Devices => midi::print_midi_devices(&mut app.output, "tune-cli")
                .handle_error("Could not print MIDI devices"),
        }
    }
}

pub fn run_in_shell_env() {
    let options = match MainOptions::try_parse() {
        Err(err) => {
            if err.use_stderr() {
                eprintln!("{err}")
            } else {
                println!("{err}");
            };
            return;
        }
        Ok(options) => options,
    };

    match executor::block_on(options.run()) {
        Ok(()) => {}
        // The BrokenPipe case occurs when stdout tries to communicate with a process that has already terminated.
        // Since tune is an idempotent tool with repeatable results, it is okay to ignore this error and terminate successfully.
        Err(CliError::IoError(err)) if err.kind() == ErrorKind::BrokenPipe => {}
        Err(err) => eprintln!("{err}"),
    }
}

pub fn run_in_wasm_env(
    args: impl IntoIterator<Item = String>,
    input: impl Read,
    output: impl Write,
    error: impl Write,
) {
    let mut app = App {
        input: Box::new(input),
        output: Box::new(output),
        error: Box::new(error),
    };

    let command = match MainCommand::try_parse_from(args) {
        Err(err) => {
            if err.use_stderr() {
                app.errln(err).unwrap()
            } else {
                app.writeln(err).unwrap()
            };
            return;
        }
        Ok(command) => command,
    };

    match executor::block_on(command.run(&mut app)) {
        Ok(()) => {}
        Err(err) => app.errln(err).unwrap(),
    }
}

struct App<'a> {
    input: Box<dyn 'a + Read>,
    output: Box<dyn 'a + Write>,
    error: Box<dyn 'a + Write>,
}

impl App<'_> {
    pub fn write(&mut self, message: impl Display) -> io::Result<()> {
        write!(self.output, "{message}")
    }

    pub fn writeln(&mut self, message: impl Display) -> io::Result<()> {
        writeln!(self.output, "{message}")
    }

    pub fn errln(&mut self, message: impl Display) -> io::Result<()> {
        writeln!(self.error, "{message}")
    }

    pub fn read(&mut self) -> &mut dyn Read {
        &mut self.input
    }
}

pub type CliResult<T = ()> = Result<T, CliError>;

pub enum CliError {
    CommandError(String),
    IoError(io::Error),
}

impl Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::CommandError(err) => write!(f, "error: {err}"),
            CliError::IoError(err) => write!(f, "I/O error: {err}"),
        }
    }
}

impl From<String> for CliError {
    fn from(v: String) -> Self {
        CliError::CommandError(v)
    }
}

impl From<io::Error> for CliError {
    fn from(v: io::Error) -> Self {
        CliError::IoError(v)
    }
}
