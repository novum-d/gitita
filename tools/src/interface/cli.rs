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
    pub(crate) fn new(message: impl Into<String>) -> Self {
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
        [command, args @ ..] if command == "publish" => {
            let dry_run = args.iter().any(|arg| arg == "--dry-run");
            Ok(Command::Publish { dry_run })
        }
        [] => Err(CliError::new(
            "missing command: expected `check` or `publish`",
        )),
        [command, ..] => Err(CliError::new(format!(
            "unknown command or arguments: `{command}`"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_command, Command};

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
    fn parses_publish_command_without_dry_run() {
        assert_eq!(
            parse_command(["publish"]),
            Ok(Command::Publish { dry_run: false })
        );
    }

    #[test]
    fn parses_publish_command_with_other_args_and_dry_run() {
        assert_eq!(
            parse_command(["publish", "something", "--dry-run"]),
            Ok(Command::Publish { dry_run: true })
        );
    }

    #[test]
    fn unknown_command_returns_error() {
        let error = parse_command(["unknown"]).expect_err("unknown command should fail");

        assert_eq!(error.to_string(), "unknown command or arguments: `unknown`");
    }
}
