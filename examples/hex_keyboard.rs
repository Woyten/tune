use std::env;
use tune::layout::IsomorphicLayout;

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
    for layout in IsomorphicLayout::find_by_edo(num_steps_per_octave) {
        println!(
            "Hex keyboard example for {num_steps_per_octave}{}-EDO",
            layout.wart()
        );
        println!();
        println!(
            "primary_step={}, secondary_step={}, num_cycles={}",
            layout.mos().primary_step(),
            layout.mos().secondary_step(),
            layout.pergen().num_cycles(),
        );
        println!();

        let mos = layout.mos().coprime();

        for y in -10i16..=10 {
            let div = y.div_euclid(2);
            let rem = y.rem_euclid(2);
            if rem == 1 {
                print!("  ");
            }
            for x in 0..20 {
                print!(
                    "{:>4}",
                    mos.get_key(x - div, y)
                        .rem_euclid(i32::from(num_steps_per_octave)),
                );
            }
            println!();
        }
        println!();
    }
}
