use std::process::Command;

#[test]
fn binary_prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_axe"))
        .output()
        .expect("failed to run axe binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Axe IDE v0.1.0"),
        "expected 'Axe IDE v0.1.0' in output, got: {stdout}"
    );
    assert!(output.status.success());
}
