use std::process::{Command, Stdio};
use std::io::Write;

#[test]
fn stdin_to_stdout() {
    let input = include_bytes!("../../../test/input-0001.png");

    let mut child = Command::new(env!("CARGO_BIN_EXE_led-gen"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input)
        .unwrap();

    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());

    let image = image::load_from_memory(&output.stdout).unwrap();

    assert_eq!(image.width(), 306);
    assert_eq!(image.height(), 210);
}

#[test]
fn stdin_to_file() {
    let input = include_bytes!("../../../test/input-0001.png");

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("output.png");
    let output_path = output.to_str().unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_led-gen"))
        .args(["--output", output_path])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input)
        .unwrap();

    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());

    let image = image::open(output_path).unwrap();

    assert_eq!(image.width(), 306);
    assert_eq!(image.height(), 210);
}

#[test]
fn file_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("output.png");

    let status = Command::new(env!("CARGO_BIN_EXE_led-gen"))
        .args([
            "../../test/input-0001.png",
            "--output",
            output.to_str().unwrap(),
        ])
        .status()
        .unwrap();

    assert!(status.success());
    assert!(output.exists());

    let image = image::open(output).unwrap();
    assert_eq!(image.width(), 306);
    assert_eq!(image.height(), 210);
}