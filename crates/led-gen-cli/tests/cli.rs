use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn stdin_to_stdout() {
    let input = include_bytes!("../../../test/input-0001.png");

    let mut child = Command::new(env!("CARGO_BIN_EXE_led-gen"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.as_mut().unwrap().write_all(input).unwrap();

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

    child.stdin.as_mut().unwrap().write_all(input).unwrap();

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

#[test]
fn file_ppm_treated_as_still_with_format() {
    // Stream auto-detection is stdin-only: a `.ppm` file argument must take
    // the single-image path so `--format` keeps working for stills.
    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.ppm");
    let output_path = dir.path().join("output.png");

    // Minimal 2x1 binary PPM.
    let mut ppm = Vec::from(b"P6\n2 1\n255\n".as_slice());
    ppm.extend_from_slice(&[255u8, 0, 0, 0, 255, 0]);
    std::fs::write(&input_path, &ppm).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_led-gen"))
        .args([
            input_path.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .status()
        .unwrap();

    assert!(status.success());

    let bytes = std::fs::read(&output_path).unwrap();
    // PNG magic: the file must NOT be a ppm stream.
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");

    // Defaults: border 10, led 4, gap 2 => 30x24 canvas for a 2x1 input.
    let image = image::open(&output_path).unwrap();
    assert_eq!(image.width(), 30);
    assert_eq!(image.height(), 24);
}

#[test]
fn stdin_ppm_stream_detected() {
    // A `P6` stream on stdin must take the streaming path (ppm out).
    let mut ppm = Vec::from(b"P6\n2 1\n255\n".as_slice());
    ppm.extend_from_slice(&[255u8, 0, 0, 0, 255, 0]);

    let mut child = Command::new(env!("CARGO_BIN_EXE_led-gen"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.as_mut().unwrap().write_all(&ppm).unwrap();

    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert_eq!(&output.stdout[..2], b"P6");
}
