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
    path::{Path, PathBuf},
    str::FromStr,
};

use est::EstOptions;
use io::Read;
use live::LiveOptions;
use mts::MtsOptions;
use scale::{DiffOptions, DumpOptions};
use shared::SclCommand;
use structopt::StructOpt;
use tune::{
    key::PianoKey,
    scala::{Kbm, KbmBuildError, KbmImportError, KbmRoot, Scl, SclBuildError, SclImportError},
};

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
    Kbm(KbmOptions),

    /// Analyze equal-step tunings
    #[structopt(name = "est")]
    Est(EstOptions),

    /// [out] Print a scale to stdout
    #[structopt(name = "scale")]
    Scale(ScaleCommand),

    /// Display details of a scale
    #[structopt(name = "dump")]
    Dump(DumpOptions),

    /// [in] Display differences between a source scale and a target scale
    #[structopt(name = "diff")]
    Diff(DiffOptions),

    /// Print or send MIDI Tuning Standard messages
    #[structopt(name = "mts")]
    Mts(MtsOptions),

    /// Enable synthesizers with Scale/Octave Tuning or pitch-bend support to be played in any scale.
    /// This is achieved by reading MIDI data from a sequencer/keyboard and sending a modified MIDI signal to the synthesizer.
    /// The sequencer/keyboard and synthesizer can be the same device. In this case, remember to disable local keyboard playback.
    #[structopt(name = "live")]
    Live(LiveOptions),

    /// List MIDI devices
    #[structopt(name = "devices")]
    Devices,
}

#[derive(StructOpt)]
enum ScaleCommand {
    /// Use a keyboard-mapping with the given reference note
    #[structopt(name = "ref-note")]
    WithRefNote {
        #[structopt(flatten)]
        kbm: KbmOptions,

        #[structopt(subcommand)]
        scl: SclCommand,
    },

    /// Use a kbm file
    #[structopt(name = "kbm-file")]
    UseKbmFile {
        /// The location of the kbm file to import
        kbm_file_location: PathBuf,

        #[structopt(subcommand)]
        scl: SclCommand,
    },

    /// Use a scale file in YAML format
    #[structopt(name = "scale-file")]
    UseScaleFile {
        /// The location of the YAML file to import
        scale_file_location: PathBuf,
    },

    /// Read a scale file from stdin in YAML format
    #[structopt(name = "stdin")]
    ReadStdin,
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
struct KbmRootOptions {
    /// Reference note that should sound at its original or a custom pitch, e.g. 69@440Hz
    ref_note: KbmRoot,

    /// root note / "middle note" of the scale if different from reference note
    #[structopt(long = "root")]
    root_note: Option<i16>,
}

#[derive(StructOpt)]
struct KbmOptions {
    #[structopt(flatten)]
    kbm_root: KbmRootOptions,

    /// Lower key bound (inclusve)
    #[structopt(long = "lo-key", default_value = "21")]
    lower_key_bound: i32,

    /// Upper key bound (inclusve)
    #[structopt(long = "up-key", default_value = "109")]
    upper_key_bound: i32,

    /// Keyboard mapping entries, e.g. 0,x,1,x,2,3,x,4,x,5,x,6
    #[structopt(long = "key-map", require_delimiter = true)]
    items: Option<Vec<Item>>,

    /// The formal octave of the keyboard mapping, e.g. n in n-EDO
    #[structopt(long = "octave", default_value = "0")]
    formal_octave: i16,
}

enum Item {
    Mapped(i16),
    Unmapped,
}

impl FromStr for Item {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if ["x", "X"].contains(&s) {
            return Ok(Item::Unmapped);
        }
        if let Ok(parsed) = s.parse() {
            return Ok(Item::Mapped(parsed));
        }
        Err("Invalid keyboard mapping entry. Should be x, X or an 16-bit signed integer".to_owned())
    }
}

pub fn run_in_shell_env(args: impl IntoIterator<Item = String>) -> CliResult<()> {
    let options = match MainOptions::from_iter_safe(args) {
        Err(err) => {
            if err.use_stderr() {
                return Err(CliError::CommandError(err.message));
            } else {
                println!("{}", err);
                return Ok(());
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

    fn execute_kbm_command(&mut self, key_map_params: KbmOptions) -> CliResult<()> {
        Ok(self.write(format_args!("{}", key_map_params.to_kbm()?.export()))?)
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

impl KbmRootOptions {
    pub fn to_kbm_root(&self) -> KbmRoot {
        match self.root_note {
            Some(root_note) => self
                .ref_note
                .shift_origin_by(i32::from(root_note) - self.ref_note.origin.midi_number()),
            None => self.ref_note,
        }
    }
}

impl KbmOptions {
    fn to_kbm(&self) -> CliResult<Kbm> {
        let mut builder = Kbm::builder(self.kbm_root.to_kbm_root()).range(
            PianoKey::from_midi_number(self.lower_key_bound)
                ..PianoKey::from_midi_number(self.upper_key_bound),
        );
        if let Some(items) = &self.items {
            for item in items {
                match item {
                    &Item::Mapped(scale_degree) => {
                        builder = builder.push_mapped_key(scale_degree);
                    }
                    Item::Unmapped => {
                        builder = builder.push_unmapped_key();
                    }
                }
            }
        }
        builder = builder.formal_octave(self.formal_octave);
        Ok(builder.build()?)
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

fn import_kbm_file(file_name: &Path) -> Result<Kbm, String> {
    let file =
        File::open(file_name).map_err(|io_err| format!("Could not read kbm file: {}", io_err))?;

    Kbm::import(file).map_err(|err| match err {
        KbmImportError::IoError(err) => format!("Could not read kbm file: {}", err),
        KbmImportError::ParseError { line_number, kind } => format!(
            "Could not parse kbm file at line {} ({:?})",
            line_number, kind
        ),
        KbmImportError::StructuralError(err) => format!("Malformed kbm file ({:?})", err),
        KbmImportError::BuildError(err) => format!("Unsupported kbm file ({:?})", err),
    })
}

fn import_scl_file(file_name: &Path) -> Result<Scl, String> {
    let file =
        File::open(file_name).map_err(|io_err| format!("Could not read scl file: {}", io_err))?;

    Scl::import(file).map_err(|err| match err {
        SclImportError::IoError(err) => format!("Could not read scl file: {}", err),
        SclImportError::ParseError { line_number, kind } => format!(
            "Could not parse scl file at line {} ({:?})",
            line_number, kind
        ),
        SclImportError::StructuralError(err) => format!("Malformed scl file ({:?})", err),
        SclImportError::BuildError(err) => format!("Unsupported scl file ({:?})", err),
    })
}
