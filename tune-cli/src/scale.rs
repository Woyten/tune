use std::fs::File;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use tune::key::PianoKey;
use tune::pitch::Pitch;
use tune::pitch::Pitched;
use tune::pitch::Ratio;
use tune::scala::KbmRoot;
use tune::scala::Scl;
use tune::tuning::KeyboardMapping;
use tune::tuning::Tuning;

use crate::dto::ScaleDto;
use crate::dto::ScaleItemDto;
use crate::dto::TuneDto;
use crate::error::ResultExt;
use crate::scala;
use crate::scala::KbmOptions;
use crate::scala::KbmRootOptions;
use crate::scala::SclCommand;
use crate::App;
use crate::CliError;
use crate::CliResult;

#[derive(Parser)]
pub(crate) enum ScaleCommand {
    /// Use a keyboard mapping with the given reference note
    #[command(name = "ref-note")]
    WithRefNote {
        #[command(flatten)]
        kbm: KbmOptions,

        #[command(subcommand)]
        scl: SclCommand,
    },

    /// Use a kbm file
    #[command(name = "kbm-file")]
    UseKbmFile {
        /// The location of the kbm file to import
        kbm_file_location: PathBuf,

        #[command(subcommand)]
        scl: SclCommand,
    },

    /// Use a scale file in YAML format
    #[command(name = "scale-file")]
    UseScaleFile {
        /// The location of the YAML file to import
        scale_file_location: PathBuf,
    },

    /// Read a scale file from stdin in YAML format
    #[command(name = "stdin")]
    ReadStdin,
}

#[derive(Parser)]
pub(crate) struct DumpOptions {
    #[command(flatten)]
    limit: LimitOptions,

    #[command(subcommand)]
    scale: ScaleCommand,
}

#[derive(Parser)]
pub(crate) struct DiffOptions {
    #[command(flatten)]
    limit: LimitOptions,

    #[command(subcommand)]
    source_scale: SourceScaleCommand,
}

#[derive(Parser)]
enum SourceScaleCommand {
    /// Use a scale file in YAML format
    #[command(name = "scale-file")]
    UseScaleFile {
        /// The location of the YAML file to import
        scale_file_location: PathBuf,

        #[command(subcommand)]
        target_scale: TargetScaleCommand,
    },

    /// Read a scale file from stdin in YAML format
    #[command(name = "stdin")]
    ReadStdin {
        #[command(subcommand)]
        target_scale: TargetScaleCommand,
    },
}

#[derive(Parser)]
enum TargetScaleCommand {
    /// Use a linear keyboard mapping with the given reference note
    #[command(name = "ref-note")]
    WithRefNote {
        #[command(flatten)]
        kbm_root: KbmRootOptions,

        #[command(subcommand)]
        scl: SclCommand,
    },
}

#[derive(Parser)]
struct LimitOptions {
    /// Largest acceptable numerator or denominator (ignoring powers of two)
    #[arg(long = "lim", default_value = "11")]
    odd_limit: u16,
}

pub(crate) struct Scale {
    pub origin: PianoKey,
    pub keys: Vec<PianoKey>,
    pub tuning: Box<dyn KeyboardMapping<PianoKey> + Send>,
}

impl Scale {
    fn from_kbm_and_scl(kbm: &KbmOptions, scl: &SclCommand) -> CliResult<Self> {
        let kbm = kbm.to_kbm()?;
        Ok(Scale {
            origin: kbm
                .kbm_root()
                .ref_key
                .plus_steps(kbm.kbm_root().root_offset),
            keys: kbm.range_iter().collect(),
            tuning: Box::new((scl.to_scl(None)?, kbm)),
        })
    }

    fn from_kbm_file_and_scl(kbm_file_location: &Path, scl: &SclCommand) -> CliResult<Self> {
        let kbm = scala::import_kbm_file(kbm_file_location)?;
        Ok(Scale {
            origin: kbm
                .kbm_root()
                .ref_key
                .plus_steps(kbm.kbm_root().root_offset),
            keys: kbm.range_iter().collect(),
            tuning: Box::new((scl.to_scl(None)?, kbm)),
        })
    }

    fn from_scale_file(scale_file_location: &Path) -> CliResult<Self> {
        let file =
            File::open(scale_file_location).display_err::<CliError>("Could not read scale file")?;
        let scale_dto = ScaleDto::read(file)?;
        Ok(Scale {
            origin: PianoKey::from_midi_number(scale_dto.root_key_midi_number),
            keys: scale_dto.keys(),
            tuning: Box::new(scale_dto.to_keyboard_mapping()),
        })
    }

    fn from_stdin(app: &mut App) -> CliResult<Self> {
        let scale_dto = ScaleDto::read(app.read())?;
        Ok(Scale {
            origin: PianoKey::from_midi_number(scale_dto.root_key_midi_number),
            keys: scale_dto.keys(),
            tuning: Box::new(scale_dto.to_keyboard_mapping()),
        })
    }
}

impl ScaleCommand {
    pub fn to_scale(&self, app: &mut App) -> CliResult<Scale> {
        match self {
            ScaleCommand::WithRefNote { kbm, scl } => Scale::from_kbm_and_scl(kbm, scl),
            ScaleCommand::UseKbmFile {
                kbm_file_location,
                scl,
            } => Scale::from_kbm_file_and_scl(kbm_file_location, scl),
            ScaleCommand::UseScaleFile {
                scale_file_location,
            } => Scale::from_scale_file(scale_file_location),
            ScaleCommand::ReadStdin => Scale::from_stdin(app),
        }
    }

    pub fn run(&self, app: &mut App) -> CliResult {
        let scale = self.to_scale(app)?;

        let items = scale
            .keys
            .iter()
            .filter_map(|&piano_key| {
                scale
                    .tuning
                    .maybe_pitch_of(piano_key)
                    .map(|pitch| ScaleItemDto {
                        key_midi_number: piano_key.midi_number(),
                        pitch_in_hz: pitch.as_hz(),
                    })
            })
            .collect();

        let dump = ScaleDto {
            root_key_midi_number: scale.origin.midi_number(),
            root_pitch_in_hz: scale.tuning.maybe_pitch_of(scale.origin).map(Pitch::as_hz),
            items,
        };

        let dto = TuneDto::Scale(dump);

        serde_yaml::to_writer(&mut app.output, &dto)
            .display_err::<CliError>("Could not write scale file")
    }
}

impl DumpOptions {
    pub fn run(&self, app: &mut App) -> CliResult {
        let scale = self.scale.to_scale(app)?;

        let mut printer = ScaleTablePrinter {
            app,
            root_key: scale.origin,
            root_pitch: scale.tuning.maybe_pitch_of(scale.origin),
            odd_limit: self.limit.odd_limit,
        };

        printer.print_table_header()?;
        for (source_key, pitch) in scale
            .keys
            .iter()
            .flat_map(|&key| scale.tuning.maybe_pitch_of(key).map(|pitch| (key, pitch)))
        {
            let approximation = pitch.find_in_tuning(());
            let (letter, octave) = approximation.approx_value.letter_and_octave();

            printer.print_table_row(
                source_key,
                pitch,
                approximation.approx_value.midi_number(),
                format!("{:>6} {:>2}", letter, octave.octave_number()),
                approximation.deviation,
            )?;
        }
        Ok(())
    }
}

impl DiffOptions {
    pub fn run(&self, app: &mut App) -> CliResult {
        let source_scale = self.source_scale.source_scale(app)?;
        let (target_scl, target_kbm_root) = self.source_scale.target_tuning()?;

        let mut printer = ScaleTablePrinter {
            app,
            root_pitch: source_scale.tuning.maybe_pitch_of(source_scale.origin),
            root_key: source_scale.origin,
            odd_limit: self.limit.odd_limit,
        };

        printer.print_table_header()?;
        for (source_key, pitch) in source_scale.keys.iter().flat_map(|&key| {
            source_scale
                .tuning
                .maybe_pitch_of(key)
                .map(|pitch| (key, pitch))
        }) {
            let approximation = (&target_scl, target_kbm_root).find_by_pitch(pitch);
            let index = target_kbm_root
                .ref_key
                .num_keys_before(approximation.approx_value);

            printer.print_table_row(
                source_key,
                pitch,
                approximation.approx_value.midi_number(),
                format!("IDX {index:>5}"),
                approximation.deviation,
            )?;
        }
        Ok(())
    }
}

impl SourceScaleCommand {
    pub fn source_scale(&self, app: &mut App) -> CliResult<Scale> {
        match self {
            SourceScaleCommand::UseScaleFile {
                scale_file_location,
                ..
            } => Scale::from_scale_file(scale_file_location),
            SourceScaleCommand::ReadStdin { .. } => Scale::from_stdin(app),
        }
    }

    pub fn target_tuning(&self) -> CliResult<(Scl, KbmRoot)> {
        let target_scale = match self {
            SourceScaleCommand::UseScaleFile { target_scale, .. } => target_scale,
            SourceScaleCommand::ReadStdin { target_scale } => target_scale,
        };

        let TargetScaleCommand::WithRefNote { kbm_root, scl } = target_scale;
        Ok((scl.to_scl(None)?, kbm_root.to_kbm_root()))
    }
}

struct ScaleTablePrinter<'a, 'b> {
    app: &'a mut App<'b>,
    root_key: PianoKey,
    root_pitch: Option<Pitch>,
    odd_limit: u16,
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

        let nearest_fraction = Ratio::between_pitches(self.root_pitch.unwrap_or(pitch), pitch)
            .nearest_fraction(self.odd_limit);

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
