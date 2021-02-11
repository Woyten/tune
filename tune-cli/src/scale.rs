use std::io;

use structopt::StructOpt;

use tune::{
    key::PianoKey,
    pitch::{Pitch, Pitched, Ratio},
    scala::{Kbm, Scl},
    tuning::{KeyboardMapping, Tuning},
};

use crate::{
    dto::{ScaleDto, ScaleItemDto, TuneDto},
    shared::SclCommand,
    App, CliResult, KbmRootOptions, ScaleCommand,
};

#[derive(StructOpt)]
pub(crate) struct DumpOptions {
    #[structopt(flatten)]
    limit: LimitOptions,
}

#[derive(StructOpt)]
pub(crate) struct DiffOptions {
    #[structopt(flatten)]
    limit: LimitOptions,

    #[structopt(flatten)]
    kbm_root: KbmRootOptions,

    #[structopt(subcommand)]
    scl: SclCommand,
}

#[derive(StructOpt)]
struct LimitOptions {
    /// Largest acceptable numerator or denominator (ignoring powers of two)
    #[structopt(long = "lim", default_value = "11")]
    limit: u16,
}

impl ScaleCommand {
    pub fn tuning(&self) -> CliResult<(Scl, Kbm)> {
        match self {
            ScaleCommand::WithRefNote(scale) => Ok((scale.scl.to_scl(None)?, scale.kbm.to_kbm()?)),
        }
    }

    pub fn run(&self, app: &mut App) -> CliResult<()> {
        let keyboard_mapping = self.tuning()?;
        let kbm = &keyboard_mapping.1;

        let items = kbm
            .range_iter()
            .filter_map(|piano_key| {
                keyboard_mapping
                    .maybe_pitch_of(piano_key)
                    .map(|pitch| ScaleItemDto {
                        key_midi_number: piano_key.midi_number(),
                        pitch_in_hz: pitch.as_hz(),
                    })
            })
            .collect();

        let dump = ScaleDto {
            root_key_midi_number: kbm.kbm_root().origin.midi_number(),
            root_pitch_in_hz: keyboard_mapping.maybe_pitch_of(0).map(Pitch::as_hz),
            items,
        };

        let dto = TuneDto::Scale(dump);

        app.writeln(format_args!(
            "{}",
            serde_json::to_string_pretty(&dto).map_err(io::Error::from)?
        ))
        .map_err(Into::into)
    }
}

impl DumpOptions {
    pub fn run(&self, app: &mut App) -> CliResult<()> {
        let in_scale = ScaleDto::read(app.read())?;

        let mut printer = ScaleTablePrinter {
            app,
            root_key: PianoKey::from_midi_number(in_scale.root_key_midi_number),
            root_pitch: in_scale.root_pitch_in_hz.map(Pitch::from_hz),
            limit: self.limit.limit,
        };

        printer.print_table_header()?;
        for scale_item in in_scale.items {
            let pitch = Pitch::from_hz(scale_item.pitch_in_hz);
            let approximation = pitch.find_in_tuning(());

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
}

impl DiffOptions {
    pub fn run(&self, app: &mut App) -> CliResult<()> {
        let in_scale = ScaleDto::read(app.read())?;

        let kbm = self.kbm_root.to_kbm_root();
        let tuning = (self.scl.to_scl(None)?, &kbm);

        let mut printer = ScaleTablePrinter {
            app,
            root_pitch: in_scale.root_pitch_in_hz.map(Pitch::from_hz),
            root_key: PianoKey::from_midi_number(in_scale.root_key_midi_number),
            limit: self.limit.limit,
        };

        printer.print_table_header()?;
        for item in in_scale.items {
            let pitch = Pitch::from_hz(item.pitch_in_hz);

            let approximation = tuning.find_by_pitch(pitch);
            let index = kbm.origin.num_keys_before(approximation.approx_value);

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
}

struct ScaleTablePrinter<'a, 'b> {
    app: &'a mut App<'b>,
    root_key: PianoKey,
    root_pitch: Option<Pitch>,
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

        let nearest_fraction = Ratio::between_pitches(self.root_pitch.unwrap_or(pitch), pitch)
            .nearest_fraction(self.limit);

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
