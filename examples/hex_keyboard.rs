use std::env;
use tune::{
    key::{Keyboard, PianoKey},
    temperament::EqualTemperament,
};

fn main() {
    let mut args = env::args();
    args.next();
    match args.next() {
        Some(num_steps_per_octave) => print_hex_keyboard(num_steps_per_octave.parse().unwrap()),
        None => {
            print_hex_keyboard(31);
            println!();
            println!("Provide command line argument to change EDO number");
        }
    };
}

pub fn print_hex_keyboard(num_steps_per_octave: u16) {
    let temperament = EqualTemperament::find().by_edo(num_steps_per_octave);
    let keyboard = Keyboard::root_at(PianoKey::from_midi_number(0))
        .with_steps_of(&temperament)
        .coprime();

    println!("Hex keyboard example for {num_steps_per_octave}-EDO");
    println!();
    println!(
        "primary_step={}, secondary_step={}, num_cycles={}",
        temperament.primary_step(),
        temperament.secondary_step(),
        temperament.num_cycles(),
    );
    println!();

    for y in (-10i16..10).rev() {
        let rem = y.div_euclid(2);
        if y % 2 == 0 {
            print!("  ");
        }
        for mut x in 0..20 {
            x += rem;
            print!(
                "{:^4}",
                keyboard
                    .get_key(x, y)
                    .midi_number()
                    .rem_euclid(i32::from(num_steps_per_octave)),
            );
        }
        println!();
    }
}
