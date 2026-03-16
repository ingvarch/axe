use std::process::Command;

#[test]
fn version_flag_prints_version_and_exits() {
    let output = Command::new(env!("CARGO_BIN_EXE_axe"))
        .arg("--version")
        .output()
        .expect("failed to run axe binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("axe 0.1.0"),
        "expected 'axe 0.1.0' in output, got: {stdout}"
    );
    assert!(output.status.success());
}

#[test]
fn short_version_flag_prints_version_and_exits() {
    let output = Command::new(env!("CARGO_BIN_EXE_axe"))
        .arg("-V")
        .output()
        .expect("failed to run axe binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("axe 0.1.0"),
        "expected 'axe 0.1.0' in output, got: {stdout}"
    );
    assert!(output.status.success());
}
