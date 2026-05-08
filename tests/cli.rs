use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn check_command_succeeds() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.arg("check").assert().success();
}

#[test]
fn publish_dry_run_command_succeeds() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.args(["publish", "--dry-run"]).assert().success();
}

#[test]
fn publish_without_dry_run_fails_with_help_message() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.arg("publish")
        .assert()
        .failure()
        .stderr(contains("publish is not implemented yet; use `publish --dry-run`"));
}

#[test]
fn unknown_command_fails_with_readable_message() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.arg("unknown")
        .assert()
        .failure()
        .stderr(contains("unknown command or arguments: `unknown`"));
}
