use std::process::{Command, Stdio};

#[test]
fn dump_7_edo() {
    let output = Command::new(env!("CARGO_BIN_EXE_tune"))
        .arg("dump")
        .arg("62")
        .arg("equal")
        .arg("1:7:2")
        .output()
        .unwrap();
    assert_output_matches(
        &output.stdout,
        include_bytes!("snapshots/dump_62_equal_1:7:2.stdout"),
    );
}

#[test]
fn jdump_7_edo() {
    let output = Command::new(env!("CARGO_BIN_EXE_tune"))
        .arg("jdump")
        .arg("62")
        .arg("equal")
        .arg("1:7:2")
        .output()
        .unwrap();
    assert_output_matches(
        &output.stdout,
        include_bytes!("snapshots/jdump_62_equal_1:7:2.stdout"),
    );
}

#[test]
fn jdump_quarter_comma_and_dump_as_31_tet_with_shift() {
    let first_command = Command::new(env!("CARGO_BIN_EXE_tune"))
        .arg("jdump")
        .arg("62")
        .arg("rank2")
        .arg("1:4:5")
        .arg("3")
        .arg("3")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let second_command = Command::new(env!("CARGO_BIN_EXE_tune"))
        .arg("dump")
        .arg("-p")
        .arg("60")
        .arg("equal")
        .arg("1:31:2")
        .stdin(first_command.stdout.unwrap())
        .output()
        .unwrap();

    assert_output_matches(
        &second_command.stdout,
        include_bytes!("snapshots/jdump_62_rank2_1:4:5_3_3.stdout.dump_-p_60_equal_1:31:2.stdout"),
    );
}

fn assert_output_matches(actual: &[u8], expected: &[u8]) {
    if actual != expected {
        panic!(
            "This output wasn't expected:\n{}",
            String::from_utf8_lossy(actual)
        )
    }
}
