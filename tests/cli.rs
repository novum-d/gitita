use std::process::Command;

#[test]
fn check_command_succeeds() {
    let output = Command::new(env!("CARGO_BIN_EXE_gitita"))
        .arg("check")
        .output()
        .expect("failed to run gitita check");

    assert!(
        output.status.success(),
        "expected check to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn publish_dry_run_command_succeeds() {
    let output = Command::new(env!("CARGO_BIN_EXE_gitita"))
        .args(["publish", "--dry-run"])
        .output()
        .expect("failed to run gitita publish --dry-run");

    assert!(
        output.status.success(),
        "expected publish --dry-run to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unknown_command_fails_with_readable_message() {
    let output = Command::new(env!("CARGO_BIN_EXE_gitita"))
        .arg("unknown")
        .output()
        .expect("failed to run gitita unknown");

    assert!(!output.status.success(), "expected unknown command to fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown command or arguments: `unknown`"),
        "expected readable unknown command message, stderr: {stderr}"
    );
}
