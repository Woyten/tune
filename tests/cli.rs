use io::Write;
use std::{
    env,
    fs::File,
    io,
    process::{Command, Output, Stdio},
};

macro_rules! check_output {
    ($file_name:literal, $actual:expr) => {
        check_output(&$actual, include_bytes!($file_name), $file_name);
    };
}

fn check_output(actual: &[u8], expected: &[u8], file_name: &str) {
    if actual != expected {
        if env::var("FIX").as_ref().map(String::as_str) == Ok("y") {
            let mut snapshot_file = File::create("tests/".to_owned() + file_name).unwrap();
            snapshot_file.write_all(&actual).unwrap();
        } else {
            panic!(
                "Unexpected output:\n\
                 {}\n\
                 The output didn't match the content of `{}`\n\
                 Auto-fix snapshots via FIX=y cargo test",
                String::from_utf8_lossy(&actual),
                file_name
            )
        }
    }
}

fn call_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tune"))
        .args(args)
        .output()
        .unwrap()
}

fn call_cli_piped(first_args: &[&str], second_args: &[&str]) -> Output {
    let first_command = Command::new(env!("CARGO_BIN_EXE_tune"))
        .args(first_args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    Command::new(env!("CARGO_BIN_EXE_tune"))
        .args(second_args)
        .stdin(first_command.stdout.unwrap())
        .output()
        .unwrap()
}

#[test]
fn create_7_edo() {
    let output = call_cli(&["scale", "62", "equal", "1:7:2"]);
    check_output!("snapshots/scale_62_equal_1:7:2.stdout", output.stdout);
}

#[test]
fn dump_7_edo() {
    let output = call_cli_piped(&["scale", "62", "equal", "1:7:2"], &["dump"]);
    check_output!(
        "snapshots/scale_62_equal_1:7:2.stdout.dump.stdout",
        output.stdout
    );
}

#[test]
fn create_quarter_comma_and_diff_with_shifted_31_edo() {
    let output = call_cli_piped(
        &["scale", "62", "rank2", "1:4:5", "3", "3"],
        &["diff", "60", "equal", "1:31:2"],
    );
    check_output!(
        "snapshots/scale_62_rank2_1:4:5_3_3.stdout.diff_60_equal_1:31:2.stdout",
        output.stdout
    );
}

#[test]
fn mts_of_19_edo() {
    let output = call_cli_piped(&["scale", "69", "equal", "1:7:2"], &["mts"]);
    check_output!(
        "snapshots/scale_69_equal_1:7:2.stdout.mts.stdout",
        output.stdout
    );
}
