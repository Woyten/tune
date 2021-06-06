mod dto;
mod est;
mod live;
mod midi;
mod mts;
mod scale;

use std::{
    fmt::{self, Debug, Display},
    fs::File,
    io::{self, Write},
    path::PathBuf,
};

use est::EstOptions;
use io::Read;
use live::LiveOptions;
use mts::MtsOptions;
use scale::{DiffOptions, DumpOptions, ScaleCommand};
use shared::{KbmOptions, SclCommand};
use structopt::StructOpt;
use tune::scala::{KbmBuildError, SclBuildError};

#[doc(hidden)]
pub mod shared;

#[derive(StructOpt)]
struct MainOptions {
    /// Write output to a file instead of stdout
    #[structopt(long = "--of")]
    output_file: Option<PathBuf>,

    #[structopt(subcommand)]
    command: MainCommand,
}

#[derive(StructOpt)]
enum MainCommand {
    /// Create a scale file
    #[structopt(name = "scl")]
    Scl(SclOptions),

    /// Create a keyboard mapping file
    #[structopt(name = "kbm")]
    Kbm(KbmCommand),

    /// Analyze equal-step tunings
    #[structopt(name = "est")]
    Est(EstOptions),

    /// Print a scale to stdout
    #[structopt(name = "scale")]
    Scale(ScaleCommand),

    /// Display details of a scale
    #[structopt(name = "dump")]
    Dump(DumpOptions),

    /// Display differences between a source scale and a target scale
    #[structopt(name = "diff")]
    Diff(DiffOptions),

    /// Print or send MIDI Tuning Standard messages to MIDI devices
    #[structopt(name = "mts")]
    Mts(MtsOptions),

    /// Enable synthesizers with limited tuning support to be played in any tuning.
    /// This is achieved by reading MIDI data from a sequencer/keyboard and sending modified MIDI data to a synthesizer.
    /// The sequencer/keyboard and synthesizer can be the same device. In this case, remember to disable local keyboard playback.
    #[structopt(name = "live")]
    Live(LiveOptions),

    /// List MIDI devices
    #[structopt(name = "devices")]
    Devices,
}

#[derive(StructOpt)]
struct SclOptions {
    /// Name of the scale
    #[structopt(long = "--name")]
    name: Option<String>,

    #[structopt(subcommand)]
    scl: SclCommand,
}

#[derive(StructOpt)]
enum KbmCommand {
    /// Provide a reference note
    #[structopt(name = "ref-note")]
    WithRefNote {
        #[structopt(flatten)]
        kbm: KbmOptions,
    },
}

pub fn run_in_shell_env(args: impl IntoIterator<Item = String>) -> CliResult<()> {
    let options = match MainOptions::from_iter_safe(args) {
        Err(err) => {
            return if err.use_stderr() {
                Err(CliError::CommandError(err.message))
            } else {
                println!("{}", err);
                Ok(())
            };
        }
        Ok(options) => options,
    };

    let stdin = io::stdin();
    let input = Box::new(stdin.lock());

    let stdout = io::stdout();
    let output: Box<dyn Write> = match options.output_file {
        Some(output_file) => Box::new(File::create(output_file)?),
        None => Box::new(stdout.lock()),
    };

    let stderr = io::stderr();
    let error = Box::new(stderr.lock());

    let mut app = App {
        input,
        output,
        error,
    };
    app.run(options.command)
}

pub fn run_in_wasm_env(
    args: impl IntoIterator<Item = String>,
    input: impl Read,
    mut output: impl Write,
    error: impl Write,
) -> CliResult<()> {
    let command = match MainCommand::from_iter_safe(args) {
        Err(err) => {
            return if err.use_stderr() {
                Err(CliError::CommandError(err.message))
            } else {
                output.write_all(err.message.as_bytes())?;
                Ok(())
            }
        }
        Ok(command) => command,
    };

    App {
        input: Box::new(input),
        output: Box::new(output),
        error: Box::new(error),
    }
    .run(command)
}

struct App<'a> {
    input: Box<dyn 'a + Read>,
    output: Box<dyn 'a + Write>,
    error: Box<dyn 'a + Write>,
}

impl App<'_> {
    fn run(&mut self, command: MainCommand) -> CliResult<()> {
        match command {
            MainCommand::Scl(SclOptions { name, scl: command }) => {
                self.execute_scl_command(name, command)?
            }
            MainCommand::Kbm(kbm) => self.execute_kbm_command(kbm)?,
            MainCommand::Est(options) => options.run(self)?,
            MainCommand::Scale(options) => options.run(self)?,
            MainCommand::Dump(options) => options.run(self)?,
            MainCommand::Diff(options) => options.run(self)?,
            MainCommand::Mts(options) => options.run(self)?,
            MainCommand::Live(options) => options.run(self)?,
            MainCommand::Devices => shared::print_midi_devices(&mut self.output, "tune-cli")?,
        }
        Ok(())
    }

    fn execute_scl_command(&mut self, name: Option<String>, command: SclCommand) -> CliResult<()> {
        Ok(self.write(format_args!("{}", command.to_scl(name)?.export()))?)
    }

    fn execute_kbm_command(
        &mut self,
        KbmCommand::WithRefNote { kbm }: KbmCommand,
    ) -> CliResult<()> {
        Ok(self.write(format_args!("{}", kbm.to_kbm()?.export()))?)
    }

    pub fn write(&mut self, message: impl Display) -> io::Result<()> {
        write!(&mut self.output, "{}", message)
    }

    pub fn writeln(&mut self, message: impl Display) -> io::Result<()> {
        writeln!(&mut self.output, "{}", message)
    }

    pub fn errln(&mut self, message: impl Display) -> io::Result<()> {
        writeln!(&mut self.error, "{}", message)
    }

    pub fn read(&mut self) -> &mut dyn Read {
        &mut self.input
    }
}

pub type CliResult<T> = Result<T, CliError>;

pub enum CliError {
    IoError(io::Error),
    CommandError(String),
}

impl Debug for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::IoError(err) => write!(f, "IO error / {}", err),
            CliError::CommandError(err) => write!(f, "The command failed / {}", err),
        }
    }
}

impl From<String> for CliError {
    fn from(v: String) -> Self {
        CliError::CommandError(v)
    }
}

impl From<SclBuildError> for CliError {
    fn from(v: SclBuildError) -> Self {
        CliError::CommandError(format!("Could not create scale ({:?})", v))
    }
}

impl From<KbmBuildError> for CliError {
    fn from(v: KbmBuildError) -> Self {
        CliError::CommandError(format!("Could not create keybord mapping ({:?})", v))
    }
}

impl From<io::Error> for CliError {
    fn from(v: io::Error) -> Self {
        CliError::IoError(v)
    }
}
