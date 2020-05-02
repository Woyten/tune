use std::process::{Command, Stdio};

macro_rules! check_output {
    ($file_name:literal, $actual:expr) => {
        let actual = $actual;
        let expected: &[_] = include_bytes!($file_name);
        if actual != expected {
            if std::env::var("FIX").as_ref().map(String::as_str) == Ok("y") {
                let mut snapshot_file =
                    std::fs::File::create(concat!("tests/", $file_name)).unwrap();
                std::io::Write::write_all(&mut snapshot_file, &actual).unwrap();
            } else {
                panic!(
                    "Unexpected output:\n\
                     {}\n\
                     The output didn't match the content of `{}`\n\
                     Auto-fix snapshots via FIX=y cargo test",
                    String::from_utf8_lossy(&actual),
                    $file_name
                )
            }
        }
    };
}

#[test]
fn dump_7_edo() {
    let output = Command::new(env!("CARGO_BIN_EXE_tune"))
        .args(&["dump", "62", "equal", "1:7:2"])
        .output()
        .unwrap();
    check_output!("snapshots/dump_62_equal_1:7:2.stdout", output.stdout);
}

#[test]
fn jdump_7_edo() {
    let output = Command::new(env!("CARGO_BIN_EXE_tune"))
        .args(&["jdump", "62", "equal", "1:7:2"])
        .output()
        .unwrap();
    check_output!("snapshots/jdump_62_equal_1:7:2.stdout", output.stdout);
}

#[test]
fn jdump_quarter_comma_and_diff_with_shifted_31_edo() {
    let first_command = Command::new(env!("CARGO_BIN_EXE_tune"))
        .args(&["jdump", "62", "rank2", "1:4:5", "3", "3"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let second_command = Command::new(env!("CARGO_BIN_EXE_tune"))
        .args(&["diff", "60", "equal", "1:31:2"])
        .stdin(first_command.stdout.unwrap())
        .output()
        .unwrap();

    check_output!(
        "snapshots/jdump_62_rank2_1:4:5_3_3.stdout.diff_60_equal_1:31:2.stdout",
        second_command.stdout
    );
}

#[test]
fn mts_of_19_edo() {
    let output = Command::new(env!("CARGO_BIN_EXE_tune"))
        .args(&["mts", "69", "equal", "1:7:2"])
        .output()
        .unwrap();
    check_output!("snapshots/mts_69_equal_1:7:2.stdout", output.stdout);
}
