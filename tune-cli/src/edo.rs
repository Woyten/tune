use std::io;
use tune::{
    generators::Meantone,
    key::{Keyboard, PianoKey},
    ratio::Ratio,
};

pub fn print_info(mut dst: impl io::Write, num_steps_per_octave: u16) -> io::Result<()> {
    let meantone = Meantone::for_edo(num_steps_per_octave);
    writeln!(
        dst,
        "---- Properties of {}-EDO ----",
        meantone.num_steps_per_octave()
    )?;
    writeln!(dst)?;

    writeln!(dst, "Number of cycles: {}", meantone.num_cycles())?;
    writeln!(
        dst,
        "1 fifth = {} EDO steps = {:#} = Pythagorean {:#}",
        meantone.num_steps_per_fifth(),
        meantone.size_of_fifth(),
        Ratio::between_ratios(Ratio::from_float(1.5), meantone.size_of_fifth()),
    )?;
    writeln!(
        dst,
        "1 primary step = {} EDO steps",
        meantone.primary_step()
    )?;
    writeln!(
        dst,
        "1 secondary step = {} EDO steps",
        meantone.secondary_step()
    )?;
    write!(dst, "1 sharp = {} EDO steps", meantone.sharpness())?;
    if meantone.sharpness() < 0 {
        writeln!(dst, " (Mavila)")?;
    } else {
        writeln!(dst)?;
    }
    writeln!(dst)?;

    writeln!(dst, "-- Keyboard layout --")?;
    let keyboard = Keyboard::root_at(PianoKey::from_midi_number(0))
        .with_steps_of(&Meantone::for_edo(num_steps_per_octave))
        .coprime();
    for y in (-5i16..5).rev() {
        for x in 0..10 {
            write!(
                dst,
                "{:^4}",
                keyboard
                    .get_key(x, y)
                    .midi_number()
                    .rem_euclid(i32::from(num_steps_per_octave)),
            )?;
        }
        writeln!(dst)?;
    }
    writeln!(dst)?;

    writeln!(dst, "-- Scale steps --")?;

    let location_of_minor_third = (Ratio::from_float(6.0 / 5.0).as_octaves()
        * f64::from(meantone.num_steps_per_octave()))
    .round() as u16;
    let location_of_major_third = (Ratio::from_float(5.0 / 4.0).as_octaves()
        * f64::from(meantone.num_steps_per_octave()))
    .round() as u16;
    let location_of_fourth = meantone.num_steps_per_octave() - meantone.num_steps_per_fifth();
    let location_of_fifth = meantone.num_steps_per_fifth();

    for index in 0..meantone.num_steps_per_octave() {
        write!(dst, "{:>3}. ", index,)?;
        write!(dst, "{}", meantone.get_heptatonic_name(i32::from(index)))?;
        if index == location_of_minor_third {
            write!(dst, " **Minor 3rd**")?;
        }
        if index == location_of_major_third {
            write!(dst, " **Major 3rd**")?;
        }
        if index == location_of_fourth {
            write!(dst, " **4th**")?;
        }
        if index == location_of_fifth {
            write!(dst, " **5th**")?;
        }
        writeln!(dst)?;
    }

    Ok(())
}
