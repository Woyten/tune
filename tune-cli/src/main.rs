mod dto;
mod edo;

use dto::{ScaleDto, ScaleItemDto, TuneDto};
use io::{ErrorKind, Read};
use scala::{SclBuildError, SclImportError};
use std::fs::File;
use std::io;
use std::io::Write;
use std::{
    fmt::{self, Arguments, Debug},
    path::PathBuf,
};
use structopt::StructOpt;
use tune::key::PianoKey;
use tune::mts::{DeviceId, SingleNoteTuningChange, SingleNoteTuningChangeMessage};
use tune::pitch::{Pitch, ReferencePitch};
use tune::ratio::{Ratio, RatioExpression, RatioExpressionVariant};
use tune::scala;
use tune::scala::Kbm;
use tune::scala::Scl;
use tune::tuning::Tuning;

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
    Scl(SclCommand),

    /// Create a keyboard mapping file
    #[structopt(name = "kbm")]
    Kbm(KbmOptions),

    /// Analzye EDO scales
    #[structopt(name = "edo")]
    Edo(EdoOptions),

    /// [out] Create a new scale
    #[structopt(name = "scale")]
    Scale(ScaleOptions),

    /// [in] Display details of a scale
    #[structopt(name = "dump")]
    Dump(DumpOptions),

    /// [in] Display differences between a source scale and a target scale
    #[structopt(name = "diff")]
    Diff(DiffOptions),

    /// [in] Dump realtime MIDI Tuning Standard messages
    #[structopt(name = "mts")]
    Mts(MtsOptions),
}

#[derive(StructOpt)]
struct EdoOptions {
    /// Number of steps per octave
    num_steps_per_octave: u16,
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
struct MtsOptions {
    /// Tuning message as binary file dump
    #[structopt(long = "bin")]
    binary_output: bool,

    /// ID of the device that should react to the tuning change
    #[structopt(long = "dev")]
    device_id: Option<u8>,

    /// Tuning program that should be affected
    #[structopt(long = "tpg", default_value = "0")]
    tuning_program: u8,
}

#[derive(StructOpt)]
enum SclCommand {
    /// Equal temperament
    #[structopt(name = "equal")]
    EqualTemperament {
        /// Step size, e.g. 1:12:2
        step_size: Ratio,
    },

    /// Rank-2 temperament
    #[structopt(name = "rank2")]
    Rank2Temperament {
        /// First generator (finite), e.g. 3/2
        generator: Ratio,

        /// Number of positive generations using the first generator, e.g. 6
        num_pos_generations: u16,

        /// Number of negative generations using the first generator, e.g. 1
        #[structopt(default_value = "0")]
        num_neg_generations: u16,

        /// Second generator (infinite)
        #[structopt(short = "p", default_value = "2")]
        period: Ratio,
    },

    /// Harmonic series
    #[structopt(name = "harm")]
    HarmonicSeries {
        /// The lowest harmonic, e.g. 8
        lowest_harmonic: u16,

        /// Number of of notes, e.g. 8
        #[structopt(short = "n")]
        number_of_notes: Option<u16>,

        /// Build subharmonic series
        #[structopt(short = "s")]
        subharmonics: bool,
    },

    /// Custom Scale
    #[structopt(name = "cust")]
    Custom {
        /// Items of the scale
        items: Vec<RatioExpression>,

        /// Name of the scale
        #[structopt(short = "n")]
        name: Option<String>,
    },

    /// Import scl file
    #[structopt(name = "file")]
    Import {
        /// The location of the file to import
        file_name: PathBuf,
    },
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

type CliResult<T> = Result<T, CliError>;

enum CliError {
    IoError(io::Error),
    CommandError(String),
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

impl Debug for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::IoError(err) => write!(f, "IO error / {}", err),
            CliError::CommandError(err) => write!(f, "The command failed / {}", err),
        }
    }
}

fn main() -> CliResult<()> {
    let options = MainOptions::from_args();

    let stdin = io::stdin();
    let input = Box::new(stdin.lock());

    let stdout = io::stdout();
    let (output, output_is_file): (Box<dyn Write>, _) = match options.output_file {
        Some(output_file) => (Box::new(File::create(output_file)?), true),
        None => (Box::new(stdout.lock()), false),
    };

    let stderr = io::stderr();
    let error = Box::new(stderr.lock());

    let mut app = App {
        input,
        output,
        error,
        output_is_file,
    };

    match app.run(options.command) {
        // The BrokenPipe case occurs when stdout tries to communicate with a process that has already terminated.
        // Since tune is an idempotent tool with repeatable results, it is okay to ignore this error and terminate successfully.
        Err(CliError::IoError(err)) if err.kind() == ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}

struct App<'a> {
    input: Box<dyn 'a + Read>,
    output: Box<dyn 'a + Write>,
    error: Box<dyn 'a + Write>,
    output_is_file: bool,
}

impl App<'_> {
    fn run(&mut self, command: MainCommand) -> CliResult<()> {
        match command {
            MainCommand::Scl(scl) => self.execute_scl_command(scl)?,
            MainCommand::Kbm(kbm) => self.execute_kbm_command(kbm)?,
            MainCommand::Edo(EdoOptions {
                num_steps_per_octave,
            }) => edo::print_info(&mut self.output, num_steps_per_octave)?,
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
            MainCommand::Mts(MtsOptions {
                binary_output,
                device_id,
                tuning_program,
            }) => self.dump_mts(binary_output, device_id, tuning_program)?,
        }
        Ok(())
    }

    fn execute_scl_command(&mut self, command: SclCommand) -> CliResult<()> {
        self.write(format_args!("{}", create_scale(command)?.export()))
            .map_err(Into::into)
    }

    fn execute_kbm_command(&mut self, key_map_params: KbmOptions) -> io::Result<()> {
        self.write(format_args!("{}", create_key_map(key_map_params).export()))
    }

    fn execute_scale_command(
        &mut self,
        key_map_params: KbmOptions,
        command: SclCommand,
    ) -> CliResult<()> {
        let key_map = create_key_map(key_map_params);
        let tuning = (&create_scale(command)?, &key_map);

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

        let key_map = create_key_map(key_map_params);
        let tuning = (create_scale(command)?, &key_map);

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

    fn dump_mts(
        &mut self,
        binary_output: bool,
        device_id: Option<u8>,
        tuning_program: u8,
    ) -> CliResult<()> {
        let scale = ScaleDto::read(&mut self.input)?;

        let tuning_changes = scale.items.iter().map(|item| {
            let approx = Pitch::from_hz(item.pitch_in_hz).find_in(&());
            SingleNoteTuningChange::new(
                item.key_midi_number as u8,
                approx.approx_value.midi_number(),
                approx.deviation,
            )
        });

        let device_id = device_id
            .map(|id| DeviceId::from(id).expect("Invalid device ID"))
            .unwrap_or_default();
        let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
            tuning_changes,
            device_id,
            tuning_program,
        )
        .unwrap();

        if binary_output {
            self.writebn(tuning_message.sysex_bytes())?;
        } else {
            for byte in tuning_message.sysex_bytes() {
                self.writeln(format_args!("0x{:02x}", byte))?;
            }
        }

        self.errln(format_args!(
            "Number of retuned notes: {}",
            tuning_message.retuned_notes().len(),
        ))?;
        self.errln(format_args!(
            "Number of out-of-range notes: {}",
            tuning_message.out_of_range_notes().len()
        ))?;
        Ok(())
    }

    fn write(&mut self, args: Arguments) -> io::Result<()> {
        self.output.write_fmt(args)
    }

    fn writeln(&mut self, args: Arguments) -> io::Result<()> {
        writeln!(&mut self.output, "{}", args)
    }

    fn writebn(&mut self, bytes: &[u8]) -> CliResult<()> {
        if self.output_is_file {
            self.output.write_all(bytes)?;
            Ok(())
        } else {
            Err(CliError::CommandError(
                "Binary output requires an explicit output file".to_owned(),
            ))
        }
    }

    fn errln(&mut self, args: Arguments) -> io::Result<()> {
        writeln!(&mut self.error, "{}", args)
    }
}

fn create_scale(command: SclCommand) -> Result<Scl, CliError> {
    Ok(match command {
        SclCommand::EqualTemperament { step_size } => {
            scala::create_equal_temperament_scale(step_size)?
        }
        SclCommand::Rank2Temperament {
            generator,
            num_pos_generations,
            num_neg_generations,
            period,
        } => scala::create_rank2_temperament_scale(
            generator,
            num_pos_generations,
            num_neg_generations,
            period,
        )?,
        SclCommand::HarmonicSeries {
            lowest_harmonic,
            number_of_notes,
            subharmonics,
        } => scala::create_harmonics_scale(
            u32::from(lowest_harmonic),
            u32::from(number_of_notes.unwrap_or(lowest_harmonic)),
            subharmonics,
        )?,
        SclCommand::Custom { items, name } => {
            create_custom_scale(items, name.unwrap_or_else(|| "Custom scale".to_string()))?
        }
        SclCommand::Import { file_name } => import_scl_file(file_name)?,
    })
}

fn create_custom_scale(items: Vec<RatioExpression>, name: String) -> Result<Scl, SclBuildError> {
    let mut scale = Scl::with_name(name);
    for item in items {
        match item.variant() {
            RatioExpressionVariant::Float { float_value } => {
                if let Some(float_value) = as_int(float_value) {
                    scale.push_int(float_value);
                    continue;
                }
            }
            RatioExpressionVariant::Fraction { numer, denom } => {
                if let (Some(numer), Some(denom)) = (as_int(numer), as_int(denom)) {
                    scale.push_fraction(numer, denom);
                    continue;
                }
            }
            _ => {}
        }
        scale.push_ratio(item.ratio());
    }
    scale.build()
}

fn as_int(float: f64) -> Option<u32> {
    let rounded = float.round();
    if (float - rounded).abs() < 1e-6 {
        Some(rounded as u32)
    } else {
        None
    }
}

fn import_scl_file(file_name: PathBuf) -> Result<Scl, String> {
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

impl From<SclBuildError> for CliError {
    fn from(v: SclBuildError) -> Self {
        CliError::CommandError(format!("Could not create scale ({:?})", v))
    }
}

fn create_key_map(key_map_params: KbmOptions) -> Kbm {
    Kbm {
        ref_pitch: key_map_params.ref_pitch,
        root_key: key_map_params
            .root_note
            .map(i32::from)
            .map(PianoKey::from_midi_number)
            .unwrap_or_else(|| key_map_params.ref_pitch.key()),
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
