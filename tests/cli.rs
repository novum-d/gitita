use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn check_command_succeeds() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.arg("check").assert().success();
}

#[test]
fn check_command_accepts_valid_article_frontmatter() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.current_dir("tests/fixtures/frontmatter-valid")
        .arg("check")
        .assert()
        .success();
}

#[test]
fn check_command_rejects_invalid_article_frontmatter() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.current_dir("tests/fixtures/frontmatter-invalid")
        .arg("check")
        .assert()
        .failure()
        .stderr(contains("missing required frontmatter field `title`"))
        .stderr(contains("frontmatter field `author` must not be empty"))
        .stderr(contains("frontmatter field `tags` must be an array"))
        .stderr(contains(
            "frontmatter field `qiita_id` must be null or a non-empty value",
        ))
        .stderr(contains("unsupported frontmatter field `published`"));
}

#[test]
fn check_command_accepts_valid_markdown_images() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.current_dir("tests/fixtures/markdown-valid")
        .arg("check")
        .assert()
        .success();
}

#[test]
fn check_command_rejects_invalid_markdown_images() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.current_dir("tests/fixtures/markdown-invalid")
        .arg("check")
        .assert()
        .failure()
        .stderr(contains("must stay inside the article directory"))
        .stderr(contains("must be relative to the article directory"))
        .stderr(contains("unsupported image extension `.svg`"));
}

#[test]
fn publish_dry_run_command_succeeds() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.env_remove("QIITA_TOKEN")
        .args(["publish", "--dry-run"])
        .assert()
        .success();
}

#[test]
fn publish_without_dry_run_fails_with_help_message() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.arg("publish").assert().failure().stderr(contains(
        "publish is not implemented yet; use `publish --dry-run`",
    ));
}

#[test]
fn unknown_command_fails_with_readable_message() {
    let mut cmd = Command::cargo_bin("gitita").expect("failed to find gitita binary");

    cmd.arg("unknown")
        .assert()
        .failure()
        .stderr(contains("unknown command or arguments: `unknown`"));
}
