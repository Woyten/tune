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
    let output = call_cli(&["scale", "62", "equal", "1:7:2"]);
    check_output!("snapshots/scale_62_equal_1_7_2.stdout", output.stdout);
}

#[test]
fn dump_7_edo() {
    let output = call_cli_piped(&["scale", "62", "equal", "1:7:2"], &["dump"]);
    check_output!(
        "snapshots/scale_62_equal_1_7_2.stdout.dump.stdout",
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
        "snapshots/scale_62_rank2_1_4_5_3_3.stdout.diff_60_equal_1_31_2.stdout",
        output.stdout
    );
}

#[test]
fn mts_of_19_edo() {
    let output = call_cli_piped(&["scale", "69", "equal", "1:7:2"], &["mts"]);
    check_output!(
        "snapshots/scale_69_equal_1_7_2.stdout.mts.stdout",
        output.stdout
    );
    check_output!(
        "snapshots/scale_69_equal_1_7_2.stdout.mts.stderr",
        output.stderr
    );
}

#[test]
fn analysis_of_15_edo() {
    let output = call_cli(&["edo", "15"]);
    check_output!("snapshots/edo_15.stdout", output.stdout);
}

#[test]
fn analysis_of_16_edo() {
    let output = call_cli(&["edo", "16"]);
    check_output!("snapshots/edo_16.stdout", output.stdout);
}

#[test]
fn crate_custom_scale() {
    let output = call_cli(&[
        "scl",
        "cust",
        "-n",
        "Just intonation",
        "9/8",
        "1.25",
        "4/3",
        "1.5",
        "5/3",
        "15/8",
        "2",
    ]);
    check_output!(
        "snapshots/scl_cust_-n_Just_intonation_9-8_5-4_4-3_3-2_5-3_15-8_2.stdout",
        output.stdout
    );
}
