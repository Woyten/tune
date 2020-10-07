use std::io;
use structopt::StructOpt;
use tune::{
    key::{Keyboard, PianoKey},
    ratio::Ratio,
    temperament::{EqualTemperament, TemperamentType},
};

use crate::App;

#[derive(StructOpt)]
pub(crate) struct EstOptions {
    /// Size of the interval to analyze
    step_size: Ratio,
}

impl EstOptions {
    pub fn run(&self, app: &mut App) -> io::Result<()> {
        let temperament = EqualTemperament::find().by_step_size(self.step_size);
        self.print_temperament(app, &temperament)?;
        match temperament.temperament_type() {
            TemperamentType::Meantone => {
                if let Some(porcupine) = temperament.as_porcupine() {
                    app.writeln("")?;
                    self.print_temperament(app, &porcupine)?;
                }
            }
            TemperamentType::Porcupine => {}
        }

        Ok(())
    }

    fn print_temperament(&self, app: &mut App, temperament: &EqualTemperament) -> io::Result<()> {
        let stretch = temperament.size_of_octave().deviation_from(Ratio::octave());
        app.writeln(format_args!(
            "---- Properties of {}-EDO{}({}) ----",
            temperament.num_steps_per_octave(),
            if stretch.is_negligible() {
                format!(" ")
            } else {
                format!(" stretched by {:#} ", stretch)
            },
            temperament.temperament_type()
        ))?;
        app.writeln("")?;
        app.writeln(format_args!(
            "Number of cycles: {}",
            temperament.num_cycles()
        ))?;
        app.writeln(format_args!(
            "1 fifth = {} EDO steps = {:#} (pythagorean {:#})",
            temperament.num_steps_per_fifth(),
            temperament.size_of_fifth(),
            temperament
                .size_of_fifth()
                .deviation_from(Ratio::from_float(1.5))
        ))?;
        app.writeln(format_args!(
            "1 primary step = {} EDO steps",
            temperament.primary_step()
        ))?;
        app.writeln(format_args!(
            "1 secondary step = {} EDO steps",
            temperament.secondary_step()
        ))?;
        app.write(format_args!(
            "1 sharp = {} EDO steps",
            temperament.sharpness()
        ))?;
        if temperament.sharpness() < 0 {
            app.write(" (Mavila)")?;
        }
        app.writeln("")?;
        app.writeln("")?;

        let keyboard = Keyboard::root_at(PianoKey::from_midi_number(0))
            .with_steps_of(&temperament)
            .coprime();

        app.writeln("-- Keyboard layout --")?;
        for y in (-5i16..5).rev() {
            for x in 0..10 {
                app.write(format_args!(
                    "{:^4}",
                    keyboard
                        .get_key(x, y)
                        .midi_number()
                        .rem_euclid(i32::from(temperament.num_steps_per_octave())),
                ))?;
            }
            app.writeln("")?;
        }
        app.writeln("")?;

        let location_of_minor_third = (Ratio::from_float(6.0 / 5.0).as_octaves()
            * f64::from(temperament.num_steps_per_octave()))
        .round() as u16;
        let location_of_major_third = (Ratio::from_float(5.0 / 4.0).as_octaves()
            * f64::from(temperament.num_steps_per_octave()))
        .round() as u16;
        let location_of_fourth =
            temperament.num_steps_per_octave() - temperament.num_steps_per_fifth();
        let location_of_fifth = temperament.num_steps_per_fifth();

        app.writeln("-- Scale steps --")?;
        for index in 0..temperament.num_steps_per_octave() {
            app.write(format_args!("{:>3}. ", index,))?;
            app.write(format_args!(
                "{}",
                temperament.get_heptatonic_name(i32::from(index))
            ))?;
            if index == location_of_minor_third {
                app.write(" **JI m3rd**")?;
            }
            if index == location_of_major_third {
                app.write(" **JI M3rd**")?;
            }
            if index == location_of_fourth {
                app.write(" **JI P4th**")?;
            }
            if index == location_of_fifth {
                app.write(" **JI P5th**")?;
            }
            app.writeln("")?;
        }

        Ok(())
    }
}
