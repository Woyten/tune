use std::{
    env, fs,
    process::{Command, Output, Stdio},
};

macro_rules! check_output {
    ($file_name:literal, $actual:expr) => {
        check_output(&$actual, include_str!($file_name), $file_name);
    };
}

fn check_output(actual: &[u8], expected: &str, file_name: &str) {
    fs::write("tests/".to_owned() + file_name, actual).unwrap();
    assert_eq!(String::from_utf8_lossy(actual), expected);
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
    let output = call_cli(&["scale", "62", "steps", "1:7:2"]);
    check_output!("snapshots/scale_62_steps_1_7_2.stdout", output.stdout);
}

#[test]
fn dump_7_edo() {
    let output = call_cli_piped(&["scale", "62", "steps", "1:7:2"], &["dump"]);
    check_output!(
        "snapshots/scale_62_steps_1_7_2.stdout.dump.stdout",
        output.stdout
    );
}

#[test]
fn create_quarter_comma_and_diff_with_shifted_31_edo() {
    let output = call_cli_piped(
        &["scale", "62", "rank2", "1:4:5", "3", "3"],
        &["diff", "60", "steps", "1:31:2"],
    );
    check_output!(
        "snapshots/scale_62_rank2_1-4-5_3_3.stdout.diff_60_steps_1_31_2.stdout",
        output.stdout
    );
}

#[test]
fn mts_of_19_edo() {
    let output = call_cli_piped(&["scale", "69", "steps", "1:7:2"], &["mts", "from-json"]);
    check_output!("snapshots/mts_of_19_edo.stdout", output.stdout);
    check_output!("snapshots/mts_of_19_edo.stderr", output.stderr);
}

#[test]
fn octave_tuning_of_31_edo() {
    let output = call_cli(&[
        "mts",
        "octave",
        "--dev-id",
        "22",
        "--lo-chan",
        "3",
        "62",
        "steps",
        "1:31:2",
    ]);
    check_output!("snapshots/octave_tuning_of_31_edo.stdout", output.stdout);
    check_output!("snapshots/octave_tuning_of_31_edo.stderr", output.stderr);
}

#[test]
fn tuning_program_change() {
    let output = call_cli(&["mts", "tun-pg", "--chan", "5", "10"]);
    check_output!("snapshots/tuning_program_change.stdout", output.stdout);
    check_output!("snapshots/tuning_program_change.stderr", output.stderr);
}

#[test]
fn tuning_bank_change() {
    let output = call_cli(&["mts", "tun-bk", "--chan", "5", "10"]);
    check_output!("snapshots/tuning_bank_change.stdout", output.stdout);
    check_output!("snapshots/tuning_bank_change.stderr", output.stderr);
}

#[test]
fn analysis_of_stretched_15_edo() {
    let output = call_cli(&["est", "1:15.1:2"]);
    check_output!(
        "snapshots/analysis_of_stretched_15_edo.stdout",
        output.stdout
    );
}

#[test]
fn analysis_of_16_edo() {
    let output = call_cli(&["est", "1:16:2"]);
    check_output!("snapshots/analysis_of_16_edo.stdout", output.stdout);
}

#[test]
fn crate_custom_scale() {
    let output = call_cli(&[
        "scl",
        "--name",
        "Just intonation",
        "steps",
        "9/8",
        "1.25",
        "4/3",
        "1.5",
        "5/3",
        "15/8",
        "2",
    ]);
    check_output!(
        "snapshots/scl_--name_Just_intonation_steps_9-8_5-4_4-3_3-2_5-3_15-8_2.stdout",
        output.stdout
    );
}
