use std::fmt::Display;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use structopt::StructOpt;
use tune::key::PianoKey;
use tune::key_map::KeyMap;
use tune::mts::SingleNoteTuningChangeMessage;
use tune::pitch::Pitched;
use tune::pitch::ReferencePitch;
use tune::ratio::Ratio;
use tune::scale;
use tune::scale::Scale;

#[derive(StructOpt)]
enum Options {
    /// Create a scale file
    #[structopt(name = "scl")]
    Scale(ScaleOptions),

    /// Create a keyboard mapping file
    #[structopt(name = "kbm")]
    KeyMap(KeyMapOptions),

    /// Dump pitches of a scale
    #[structopt(name = "dump")]
    Dump(DumpOptions),

    // Dump MIDI tuning messages
    #[structopt(name = "mts")]
    Mts(DumpOptions),
}

#[derive(StructOpt)]
struct ScaleOptions {
    #[structopt(flatten)]
    output_file_params: OutputFileParams,

    #[structopt(subcommand)]
    command: ScaleCommand,
}

#[derive(StructOpt)]
struct KeyMapOptions {
    #[structopt(flatten)]
    output_file_params: OutputFileParams,

    #[structopt(flatten)]
    key_map_params: KeyMapParams,
}

#[derive(StructOpt)]
struct DumpOptions {
    #[structopt(flatten)]
    key_map_params: KeyMapParams,

    #[structopt(subcommand)]
    command: ScaleCommand,
}

#[derive(StructOpt)]
enum ScaleCommand {
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
        items: Vec<Ratio>,

        /// Name of the scale
        #[structopt(short = "n")]
        name: Option<String>,
    },
}

#[derive(StructOpt)]
struct OutputFileParams {
    /// Write output to file
    #[structopt(short = "o")]
    output_file: Option<PathBuf>,
}

#[derive(StructOpt)]
struct KeyMapParams {
    /// Reference note that should sound at its original or a custom pitch, e.g. 69@440Hz
    ref_pitch: ReferencePitch,

    /// root note / "middle note" of the scale if different from reference note
    #[structopt(short = "r")]
    root_note: Option<i16>,
}

fn main() -> io::Result<()> {
    match Options::from_args() {
        Options::Scale(ScaleOptions {
            output_file_params,
            command,
        }) => execute_scale_command(output_file_params, command),
        Options::KeyMap(KeyMapOptions {
            output_file_params,
            key_map_params,
        }) => execute_key_map_command(output_file_params, key_map_params),
        Options::Dump(DumpOptions {
            key_map_params,
            command,
        }) => {
            dump_scale(key_map_params, command);
            Ok(())
        }
        Options::Mts(DumpOptions {
            key_map_params,
            command,
        }) => {
            dump_mts(key_map_params, command);
            Ok(())
        }
    }
}

fn execute_scale_command(
    output_file_params: OutputFileParams,
    command: ScaleCommand,
) -> io::Result<()> {
    generate_output(output_file_params, create_scale(command).as_scl())
}

fn execute_key_map_command(
    output_file_params: OutputFileParams,
    key_map_params: KeyMapParams,
) -> io::Result<()> {
    generate_output(output_file_params, create_key_map(key_map_params).as_kbm())
}

fn dump_scale(key_map_params: KeyMapParams, command: ScaleCommand) {
    let scale = create_scale(command);
    let key_map = create_key_map(key_map_params);

    for i in 0..128 {
        println!(
            "{} | {}",
            i,
            (&scale, &key_map, PianoKey::from_midi_number(i))
                .pitch()
                .describe(Default::default())
        );
    }
}

fn dump_mts(key_map_params: KeyMapParams, command: ScaleCommand) {
    let scale = create_scale(command);
    let key_map = create_key_map(key_map_params);

    let tuning_message =
        SingleNoteTuningChangeMessage::from_scale(&scale, &key_map, Default::default()).unwrap();

    for byte in tuning_message.sysex_bytes() {
        println!("0x{:02x}", byte);
    }

    println!(
        "Number of retuned notes: {}",
        tuning_message.retuned_notes().len()
    );
    println!(
        "Number of out-of-range notes: {}",
        tuning_message.out_of_range_notes().len()
    );
}

fn create_scale(command: ScaleCommand) -> Scale {
    match command {
        ScaleCommand::EqualTemperament { step_size } => {
            scale::create_equal_temperament_scale(step_size)
        }
        ScaleCommand::Rank2Temperament {
            generator,
            num_pos_generations,
            num_neg_generations,
            period,
        } => scale::create_rank2_temperament_scale(
            generator,
            num_pos_generations,
            num_neg_generations,
            period,
        ),
        ScaleCommand::HarmonicSeries {
            lowest_harmonic,
            number_of_notes,
            subharmonics,
        } => scale::create_harmonics_scale(
            u32::from(lowest_harmonic),
            u32::from(number_of_notes.unwrap_or(lowest_harmonic)),
            subharmonics,
        ),
        ScaleCommand::Custom { items, name } => {
            create_custom_scale(items, name.unwrap_or_else(|| "Custom scale".to_string()))
        }
    }
}

fn create_custom_scale(items: Vec<Ratio>, name: String) -> Scale {
    let mut scale = Scale::with_name(name);
    for item in items {
        scale.push_ratio(item);
    }
    scale.build()
}

fn create_key_map(key_map_params: KeyMapParams) -> KeyMap {
    KeyMap {
        ref_pitch: key_map_params.ref_pitch,
        root_key: key_map_params
            .root_note
            .map(i32::from)
            .map(PianoKey::from_midi_number)
            .unwrap_or_else(|| key_map_params.ref_pitch.key()),
    }
}

fn generate_output<D: Display>(output_file_params: OutputFileParams, content: D) -> io::Result<()> {
    if let Some(output_file) = output_file_params.output_file {
        File::create(output_file).and_then(|mut file| write!(file, "{}", content))
    } else {
        print!("{}", content);
        Ok(())
    }
}
