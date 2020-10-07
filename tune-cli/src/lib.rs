mod dto;
mod est;
mod live;
mod midi;
mod mts;

use dto::{ScaleDto, ScaleItemDto, TuneDto};
use est::EstOptions;
use io::Read;
use live::LiveOptions;
use mts::MtsOptions;
use shared::SclCommand;
use std::{fmt::Display, fs::File};
use std::{
    fmt::{self, Debug},
    io::{self, Write},
    path::PathBuf,
};
use structopt::StructOpt;
use tune::key::PianoKey;
use tune::pitch::{Pitch, ReferencePitch};
use tune::ratio::Ratio;
use tune::scala::{Kbm, SclBuildError};
use tune::tuning::Tuning;

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

    /// [out] Create a new scale
    #[structopt(name = "scale")]
    Scale(ScaleOptions),

    /// [in] Display details of a scale
    #[structopt(name = "dump")]
    Dump(DumpOptions),

    /// [in] Display differences between a source scale and a target scale
    #[structopt(name = "diff")]
    Diff(DiffOptions),

    /// Print or send MIDI Tuning Standard messages
    #[structopt(name = "mts")]
    Mts(MtsOptions),

    /// Enable synthesizers with octave-based tuning support to play any octave-repeating scale.
    /// This is achieved by reading MIDI data from a sequencer/keyboard and sending a modified MIDI signal to the synthesizer.
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
    command: SclCommand,
}

#[derive(StructOpt)]
struct DumpOptions {
    #[structopt(flatten)]
    limit_params: LimitOptions,
}

#[derive(StructOpt)]
struct ScaleOptions {
    #[structopt(flatten)]
    kbm_params: KbmOptions,

    #[structopt(subcommand)]
    command: SclCommand,
}

#[derive(StructOpt)]
struct DiffOptions {
    #[structopt(flatten)]
    key_map_params: KbmOptions,

    #[structopt(flatten)]
    limit_params: LimitOptions,

    #[structopt(subcommand)]
    command: SclCommand,
}

#[derive(StructOpt)]
struct KbmOptions {
    /// Reference note that should sound at its original or a custom pitch, e.g. 69@440Hz
    ref_pitch: ReferencePitch,

    /// root note / "middle note" of the scale if different from reference note
    #[structopt(short = "r")]
    root_note: Option<i16>,
}

#[derive(StructOpt)]
struct LimitOptions {
    /// Largest acceptable numerator or denominator (ignoring powers of two)
    #[structopt(short = "l", default_value = "11")]
    limit: u16,
}

pub fn run_in_shell_env(args: impl IntoIterator<Item = String>) -> CliResult<()> {
    let options = MainOptions::from_iter_safe(args).map_err(|error| error.message)?;

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
            MainCommand::Scl(SclOptions { name, command }) => {
                self.execute_scl_command(name, command)?
            }
            MainCommand::Kbm(kbm) => self.execute_kbm_command(kbm)?,
            MainCommand::Est(options) => options.run(self)?,
            MainCommand::Scale(ScaleOptions {
                kbm_params,
                command,
            }) => self.execute_scale_command(kbm_params, command)?,
            MainCommand::Dump(DumpOptions { limit_params }) => {
                self.dump_scale(limit_params.limit)?
            }
            MainCommand::Diff(DiffOptions {
                limit_params,
                key_map_params,
                command,
            }) => self.diff_scale(key_map_params, limit_params.limit, command)?,
            MainCommand::Mts(options) => options.run(self)?,
            MainCommand::Live(options) => options.run(self)?,
            MainCommand::Devices => shared::print_midi_devices(&mut self.output, "tune-cli")?,
        }
        Ok(())
    }

    fn execute_scl_command(&mut self, name: Option<String>, command: SclCommand) -> CliResult<()> {
        self.write(format_args!("{}", command.to_scl(name)?.export()))
            .map_err(Into::into)
    }

    fn execute_kbm_command(&mut self, key_map_params: KbmOptions) -> io::Result<()> {
        self.write(format_args!("{}", key_map_params.to_kbm().export()))
    }

    fn execute_scale_command(
        &mut self,
        key_map_params: KbmOptions,
        command: SclCommand,
    ) -> CliResult<()> {
        let key_map = key_map_params.to_kbm();
        let tuning = (&command.to_scl(None)?, &key_map);

        let items = scale_iter(tuning)
            .map(|scale_item| ScaleItemDto {
                key_midi_number: scale_item.piano_key.midi_number(),
                pitch_in_hz: scale_item.pitch.as_hz(),
            })
            .collect();

        let dump = ScaleDto {
            root_key_midi_number: key_map.root_key.midi_number(),
            root_pitch_in_hz: tuning.pitch_of(0).as_hz(),
            items,
        };

        let dto = TuneDto::Scale(dump);

        self.writeln(format_args!(
            "{}",
            serde_json::to_string_pretty(&dto).map_err(io::Error::from)?
        ))
        .map_err(Into::into)
    }

    fn dump_scale(&mut self, limit: u16) -> io::Result<()> {
        let in_scale = ScaleDto::read(&mut self.input)?;

        let mut printer = ScaleTablePrinter {
            app: self,
            root_key: PianoKey::from_midi_number(in_scale.root_key_midi_number),
            root_pitch: Pitch::from_hz(in_scale.root_pitch_in_hz),
            limit,
        };

        printer.print_table_header()?;
        for scale_item in in_scale.items {
            let pitch = Pitch::from_hz(scale_item.pitch_in_hz);
            let approximation = pitch.find_in(&());

            let approx_value = approximation.approx_value;
            let (letter, octave) = approx_value.letter_and_octave();
            printer.print_table_row(
                PianoKey::from_midi_number(scale_item.key_midi_number),
                pitch,
                approx_value.midi_number(),
                format!("{:>6} {:>2}", letter, octave.octave_number()),
                approximation.deviation,
            )?;
        }
        Ok(())
    }

    fn diff_scale(
        &mut self,
        key_map_params: KbmOptions,
        limit: u16,
        command: SclCommand,
    ) -> CliResult<()> {
        let in_scale = ScaleDto::read(&mut self.input)?;

        let key_map = key_map_params.to_kbm();
        let tuning = (command.to_scl(None)?, &key_map);

        let mut printer = ScaleTablePrinter {
            app: self,
            root_pitch: Pitch::from_hz(in_scale.root_pitch_in_hz),
            root_key: PianoKey::from_midi_number(in_scale.root_key_midi_number),
            limit,
        };

        printer.print_table_header()?;
        for item in in_scale.items {
            let pitch = Pitch::from_hz(item.pitch_in_hz);

            let approximation = tuning.find_by_pitch(pitch);
            let index = key_map.root_key.num_keys_before(approximation.approx_value);

            printer.print_table_row(
                PianoKey::from_midi_number(item.key_midi_number),
                pitch,
                approximation.approx_value.midi_number(),
                format!("IDX {:>5}", index),
                approximation.deviation,
            )?;
        }
        Ok(())
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

impl KbmOptions {
    pub fn to_kbm(&self) -> Kbm {
        Kbm {
            ref_pitch: self.ref_pitch,
            root_key: self
                .root_note
                .map(i32::from)
                .map(PianoKey::from_midi_number)
                .unwrap_or_else(|| self.ref_pitch.key()),
        }
    }
}

fn scale_iter(tuning: impl Tuning<PianoKey>) -> impl Iterator<Item = ScaleItem> {
    (1..128).map(move |midi_number| {
        let piano_key = PianoKey::from_midi_number(midi_number);
        ScaleItem {
            piano_key,
            pitch: tuning.pitch_of(piano_key),
        }
    })
}

struct ScaleItem {
    piano_key: PianoKey,
    pitch: Pitch,
}

struct ScaleTablePrinter<'a, 'b> {
    app: &'a mut App<'b>,
    root_key: PianoKey,
    root_pitch: Pitch,
    limit: u16,
}

impl ScaleTablePrinter<'_, '_> {
    fn print_table_header(&mut self) -> io::Result<()> {
        self.app.writeln(format_args!(
            "  {source:-^33} ‖ {pitch:-^14} ‖ {target:-^28}",
            source = "Source Scale",
            pitch = "Pitch",
            target = "Target Scale"
        ))
    }

    fn print_table_row(
        &mut self,
        source_key: PianoKey,
        pitch: Pitch,
        target_midi: i32,
        target_index: String,
        deviation: Ratio,
    ) -> io::Result<()> {
        let source_index = self.root_key.num_keys_before(source_key);
        if source_index == 0 {
            self.app.write(format_args!("> "))?;
        } else {
            self.app.write(format_args!("  "))?;
        }

        let nearest_fraction =
            Ratio::between_pitches(self.root_pitch, pitch).nearest_fraction(self.limit);

        self.app.writeln(format_args!(
            "{source_midi:>3} | IDX {source_index:>4} | \
             {numer:>2}/{denom:<2} {fract_deviation:>+4.0}¢ {fract_octaves:>+3}o ‖ \
             {pitch:>11.3} Hz ‖ {target_midi:>4} | {target_index} | {deviation:>+8.3}¢",
            source_midi = source_key.midi_number(),
            source_index = source_index,
            pitch = pitch.as_hz(),
            numer = nearest_fraction.numer,
            denom = nearest_fraction.denom,
            fract_deviation = nearest_fraction.deviation.as_cents(),
            fract_octaves = nearest_fraction.num_octaves,
            target_midi = target_midi,
            target_index = target_index,
            deviation = deviation.as_cents(),
        ))
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

impl From<io::Error> for CliError {
    fn from(v: io::Error) -> Self {
        CliError::IoError(v)
    }
}
