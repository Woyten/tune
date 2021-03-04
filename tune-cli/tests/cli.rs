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
    let output = call_cli(&[
        "scale", "ref-note", "62", "--lo-key", "61", "--up-key", "64", "steps", "1:7:2",
    ]);
    check_output!("snapshots/README_create_7_edo.stdout", output.stdout);
}

#[test]
fn dump_7_edo() {
    let output = call_cli(&[
        "dump", "ref-note", "62", "--lo-key", "61", "--up-key", "71", "steps", "1:7:2",
    ]);
    check_output!("snapshots/README_dump_7_edo.stdout", output.stdout);
}

#[test]
fn dump_19_edo() {
    let output = call_cli(&[
        "dump", "ref-note", "62", "--lo-key", "61", "--up-key", "71", "steps", "1:19:2",
    ]);
    check_output!("snapshots/README_dump_19_edo.stdout", output.stdout);
}

#[test]
fn dump_7_edo_with_root() {
    let output = call_cli(&["dump", "ref-note", "62", "--root", "60", "steps", "1:7:2"]);
    check_output!("snapshots/dump_7_edo_with_root.stdout", output.stdout);
}

#[test]
fn diff_quarter_comma_and_31_edo() {
    let output = call_cli_piped(
        &[
            "scale", "ref-note", "62", "--lo-key", "61", "--up-key", "71", "rank2", "1:4:5", "5",
            "1",
        ],
        &["diff", "stdin", "ref-note", "62", "steps", "1:31:2"],
    );
    check_output!(
        "snapshots/README_diff_quarter_comma_and_31_edo.stdout",
        output.stdout
    );
}

#[test]
fn diff_quarter_comma_and_31_edo_with_shift() {
    let output = call_cli_piped(
        &["scale", "ref-note", "62", "rank2", "1:4:5", "3", "3"],
        &["diff", "stdin", "ref-note", "60", "steps", "1:31:2"],
    );
    check_output!(
        "snapshots/diff_quarter_comma_and_31_edo_with_shift.stdout",
        output.stdout
    );
}

#[test]
fn mts_of_7_edo() {
    let output = call_cli(&["mts", "full", "ref-note", "62", "steps", "1:7:2"]);
    check_output!("snapshots/README_mts_of_7_edo.stdout", output.stdout);
    check_output!("snapshots/README_mts_of_7_edo.stderr", output.stderr);
}

#[test]
fn mts_of_19_edo() {
    let output = call_cli(&["mts", "full", "ref-note", "69", "steps", "1:19:2"]);
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
        "ref-note",
        "62",
        "steps",
        "1:31:2",
    ]);
    check_output!("snapshots/octave_tuning_of_31_edo.stdout", output.stdout);
    check_output!("snapshots/octave_tuning_of_31_edo.stderr", output.stderr);
}

#[test]
fn octave_tuning_of_13_edt() {
    let output = call_cli(&[
        "mts",
        "octave",
        "--dev-id",
        "22",
        "--lo-chan",
        "3",
        "ref-note",
        "62",
        "steps",
        "1:13:3",
    ]);
    check_output!("snapshots/octave_tuning_of_13_edt.stdout", output.stdout);
    check_output!("snapshots/octave_tuning_of_13_edt.stderr", output.stderr);
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
fn create_scl() {
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
    check_output!("snapshots/README_create_scl.stdout", output.stdout);
}

#[test]
fn create_kbm_root() {
    let output = call_cli(&["kbm", "ref-note", "62"]);
    check_output!("snapshots/README_create_kbm_root.stdout", output.stdout);
}

#[test]
fn crate_kbm() {
    let output = call_cli(&[
        "kbm",
        "ref-note",
        "62",
        "--root",
        "60",
        "--lo-key",
        "10",
        "--up-key",
        "100",
        "--key-map",
        "0,x,1,x,2,3,x,4,x,5,x,6",
        "--octave",
        "7",
    ]);
    check_output!("snapshots/README_create_kbm.stdout", output.stdout);
}
