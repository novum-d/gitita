mod application;
mod domain;
mod infrastructure;
mod interface;

use application::check::check_articles;
use application::publish::publish_dry_run;
use infrastructure::filesystem::FileSystemArticleRepository;
use std::path::Path;

pub use interface::cli::{parse_command, CliError, Command};

pub fn run<I, S>(args: I) -> Result<(), CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    match parse_command(args)? {
        Command::Check => {
            let repository = FileSystemArticleRepository;
            check_articles(&repository, Path::new(".")).map_err(CliError::new)
        }
        Command::Publish { dry_run: true } => publish_dry_run().map_err(CliError::new),
        Command::Publish { dry_run: false } => Err(CliError::new(
            "publish is not implemented yet; use `publish --dry-run`",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn production_publish_returns_clear_error() {
        let error = run(["publish"]).expect_err("publish without dry-run should fail");

        assert_eq!(
            error.to_string(),
            "publish is not implemented yet; use `publish --dry-run`"
        );
    }
}
