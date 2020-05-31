use std::env;
use tune::{
    generators::Meantone,
    key::{Keyboard, PianoKey},
};

fn main() {
    let mut args = env::args();
    args.next();
    match args.next() {
        Some(num_divisions) => print_hex_keyboard(num_divisions.parse().unwrap()),
        None => {
            print_hex_keyboard(31);
            println!();
            println!("Provide command line argument to change EDO number");
        }
    };
}

pub fn print_hex_keyboard(num_divisions: u16) {
    let meantone = Meantone::for_edo(num_divisions);
    let keyboard = Keyboard::root_at(PianoKey::from_midi_number(0))
        .with_steps_of(&meantone)
        .coprime();

    println!("Hex keyboard example for {}-EDO", num_divisions);
    println!();
    println!(
        "primary_step={}, secondary_step={}, num_cycles={}",
        meantone.primary_step(),
        meantone.secondary_step(),
        meantone.num_cycles(),
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
                    .rem_euclid(i32::from(num_divisions)),
            );
        }
        println!();
    }
}
