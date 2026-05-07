use std::error::Error;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Check,
    Publish { dry_run: bool },
}

#[derive(Debug, PartialEq, Eq)]
pub struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for CliError {}

pub fn run<I, S>(args: I) -> Result<(), CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    match parse_command(args)? {
        Command::Check => check(),
        Command::Publish { dry_run: true } => publish_dry_run(),
        Command::Publish { dry_run: false } => Err(CliError::new(
            "publish is not implemented yet; use `publish --dry-run`",
        )),
    }
}

pub fn parse_command<I, S>(args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_owned())
        .collect();

    match args.as_slice() {
        [command] if command == "check" => Ok(Command::Check),
        [command, flag] if command == "publish" && flag == "--dry-run" => {
            Ok(Command::Publish { dry_run: true })
        }
        [command] if command == "publish" => Ok(Command::Publish { dry_run: false }),
        [] => Err(CliError::new(
            "missing command: expected `check` or `publish`",
        )),
        [command, ..] => Err(CliError::new(format!(
            "unknown command or arguments: `{command}`"
        ))),
    }
}

fn check() -> Result<(), CliError> {
    Ok(())
}

fn publish_dry_run() -> Result<(), CliError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_command, run, Command};

    #[test]
    fn parses_check_command() {
        assert_eq!(parse_command(["check"]), Ok(Command::Check));
    }

    #[test]
    fn parses_publish_dry_run_command() {
        assert_eq!(
            parse_command(["publish", "--dry-run"]),
            Ok(Command::Publish { dry_run: true })
        );
    }

    #[test]
    fn production_publish_returns_clear_error() {
        let error = run(["publish"]).expect_err("publish without dry-run should fail");

        assert_eq!(
            error.to_string(),
            "publish is not implemented yet; use `publish --dry-run`"
        );
    }

    #[test]
    fn unknown_command_returns_error() {
        let error = parse_command(["unknown"]).expect_err("unknown command should fail");

        assert_eq!(error.to_string(), "unknown command or arguments: `unknown`");
    }
}
