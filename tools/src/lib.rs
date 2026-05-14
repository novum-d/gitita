use clap::{Arg, ArgAction, Command as ClapCommand};
use serde::Deserialize;
use serde_yaml::Value;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub mod diff;
pub mod markdown;
pub mod qiita;
pub mod workflow;

#[derive(Debug, Deserialize)]
pub(crate) struct Frontmatter {
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) tags: Option<Vec<Value>>,
    #[serde(default)]
    pub(crate) author: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_as_some_null")]
    pub(crate) qiita_id: Option<Value>,

    #[serde(default)]
    pub(crate) published: Option<Value>,
}

fn deserialize_null_as_some_null<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Check,
    Publish { dry_run: bool, preview: bool },
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
        Command::Publish { dry_run, preview } => publish(dry_run, preview),
    }
}

pub fn parse_command<I, S>(args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut argv = vec!["gitita".to_owned()];
    argv.extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));

    let matches = clap_app()
        .try_get_matches_from(argv)
        .map_err(|error| CliError::new(error.to_string()))?;

    match matches.subcommand() {
        Some(("check", _)) => Ok(Command::Check),
        Some(("publish", sub_matches)) => {
            let dry_run = sub_matches.get_flag("dry-run");
            let preview = sub_matches.get_flag("preview");
            Ok(Command::Publish { dry_run, preview })
        }
        _ => Err(CliError::new(
            "missing command: expected `check` or `publish`",
        )),
    }
}

fn clap_app() -> ClapCommand {
    ClapCommand::new("gitita")
        .subcommand_required(true)
        .subcommand(ClapCommand::new("check"))
        .subcommand(
            ClapCommand::new("publish")
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("preview")
                        .long("preview")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("dry-run"),
                ),
        )
}

fn check() -> Result<(), CliError> {
    validate_articles(Path::new("."))
}

fn validate_articles(root: &Path) -> Result<(), CliError> {
    let article_paths = discover_article_paths(root)?;
    let mut errors = Vec::new();

    for path in article_paths {
        if let Err(error) = validate_article(&path) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(CliError::new(errors.join("\n")))
    }
}

fn discover_article_paths(root: &Path) -> Result<Vec<PathBuf>, CliError> {
    let articles_dir = root.join("articles");

    if !articles_dir.exists() {
        return Ok(Vec::new());
    }

    if !articles_dir.is_dir() {
        return Err(CliError::new("articles path exists but is not a directory"));
    }

    let entries = fs::read_dir(&articles_dir).map_err(|error| {
        CliError::new(format!(
            "failed to read articles directory `{}`: {error}",
            articles_dir.display()
        ))
    })?;

    let mut paths = Vec::new();

    for entry in entries {
        let entry = entry
            .map_err(|error| CliError::new(format!("failed to read article entry: {error}")))?;

        let path = entry.path();

        if path.is_dir() {
            let article_path = path.join("article.md");

            if article_path.is_file() {
                paths.push(article_path);
            }
        }
    }

    paths.sort();
    Ok(paths)
}

pub(crate) fn validate_article(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("{}: failed to read article: {error}", path.display()))?;

    let frontmatter = parse_frontmatter(path, &content)?;

    validate_required_string(path, "title", frontmatter.title.as_deref())?;
    validate_tags(path, frontmatter.tags.as_ref())?;
    validate_required_string(path, "author", frontmatter.author.as_deref())?;
    validate_qiita_id(path, &frontmatter)?;
    markdown::validate_image_references(path, &content).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    Ok(())
}

fn publish(dry_run: bool, preview: bool) -> Result<(), CliError> {
    let options = workflow::PublishOptions::from_env(dry_run, preview);
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|error| CliError::new(format!("failed to start async runtime: {error}")))?;

    runtime
        .block_on(workflow::publish(Path::new("."), options))
        .map_err(|error| CliError::new(error.to_string()))
}

pub(crate) fn parse_frontmatter(path: &Path, content: &str) -> Result<Frontmatter, String> {
    let (yaml, _) = split_frontmatter(path, content)?;
    serde_yaml::from_str::<Frontmatter>(yaml).map_err(|error| {
        let error_msg = error.to_string();
        if error_msg.contains("invalid type: string") && error_msg.contains("expected a sequence") {
            format!(
                "{}: frontmatter field `tags` must be an array",
                path.display()
            )
        } else {
            format!(
                "{}: failed to parse frontmatter yaml: {error}",
                path.display()
            )
        }
    })
}

pub(crate) fn split_frontmatter<'a>(
    path: &Path,
    content: &'a str,
) -> Result<(&'a str, &'a str), String> {
    let mut lines = content.lines();

    if lines.next() != Some("---") {
        return Err(format!(
            "{}: missing frontmatter opening delimiter `---`",
            path.display()
        ));
    }

    let frontmatter_start = content
        .find('\n')
        .map(|index| index + 1)
        .unwrap_or(content.len());
    let Some(closing_offset) = content[frontmatter_start..].find("\n---") else {
        return Err(format!(
            "{}: missing frontmatter closing delimiter `---`",
            path.display()
        ));
    };

    let frontmatter_end = frontmatter_start + closing_offset;
    let delimiter_start = frontmatter_end + 1;
    let delimiter_end = delimiter_start + 3;

    let body_start = if content[delimiter_end..].starts_with("\r\n") {
        delimiter_end + 2
    } else if content[delimiter_end..].starts_with('\n') {
        delimiter_end + 1
    } else {
        delimiter_end
    };

    Ok((
        &content[frontmatter_start..frontmatter_end],
        &content[body_start..],
    ))
}

fn validate_required_string(path: &Path, field: &str, value: Option<&str>) -> Result<(), String> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(()),
        Some(_) => Err(format!(
            "{}: frontmatter field `{field}` must not be empty",
            path.display()
        )),
        None => Err(format!(
            "{}: missing required frontmatter field `{field}`",
            path.display()
        )),
    }
}

fn validate_tags(path: &Path, tags: Option<&Vec<Value>>) -> Result<(), String> {
    match tags {
        Some(_) => Ok(()),
        None => Err(format!(
            "{}: missing required frontmatter field `tags`",
            path.display()
        )),
    }
}

fn validate_qiita_id(path: &Path, frontmatter: &Frontmatter) -> Result<(), String> {
    if frontmatter.published.is_some() {
        return Err(format!(
            "{}: unsupported frontmatter field `published`",
            path.display()
        ));
    }

    match &frontmatter.qiita_id {
        Some(Value::Null) => Ok(()),

        Some(Value::String(value)) if !value.trim().is_empty() => Ok(()),

        Some(_) => Err(format!(
            "{}: frontmatter field `qiita_id` must be null or a non-empty value",
            path.display()
        )),

        None => Err(format!(
            "{}: missing required frontmatter field `qiita_id`",
            path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_command, split_frontmatter, validate_articles, Command};
    use std::path::Path;

    #[test]
    fn parses_check_command() {
        assert_eq!(parse_command(["check"]), Ok(Command::Check));
    }

    #[test]
    fn parses_publish_dry_run_command() {
        assert_eq!(
            parse_command(["publish", "--dry-run"]),
            Ok(Command::Publish {
                dry_run: true,
                preview: false
            })
        );
    }

    #[test]
    fn parses_publish_command_without_dry_run() {
        assert_eq!(
            parse_command(["publish"]),
            Ok(Command::Publish {
                dry_run: false,
                preview: false
            })
        );
    }

    #[test]
    fn parses_publish_command_with_other_args_and_dry_run() {
        let error = parse_command(["publish", "something", "--dry-run"])
            .expect_err("unexpected args should fail");
        assert!(error.to_string().contains("unexpected argument"));
    }

    #[test]
    fn parses_publish_preview_command() {
        assert_eq!(
            parse_command(["publish", "--preview"]),
            Ok(Command::Publish {
                dry_run: false,
                preview: true
            })
        );
    }

    #[test]
    fn unknown_command_returns_error() {
        let error = parse_command(["unknown"]).expect_err("unknown command should fail");

        assert!(error
            .to_string()
            .contains("unrecognized subcommand 'unknown'"));
    }

    #[test]
    fn validates_fixture_articles_with_valid_frontmatter() {
        let fixture = Path::new("tests/fixtures/frontmatter-valid");

        validate_articles(fixture).expect("valid fixture articles should pass");
    }

    #[test]
    fn rejects_fixture_articles_with_invalid_frontmatter() {
        let fixture = Path::new("tests/fixtures/frontmatter-invalid");
        let error = validate_articles(fixture).expect_err("invalid fixture articles should fail");

        let message = error.to_string();

        assert!(message.contains("missing required frontmatter field `title`"));
        assert!(message.contains("frontmatter field `author` must not be empty"));
        assert!(message.contains("frontmatter field `tags` must be an array"));
        assert!(message.contains("frontmatter field `qiita_id` must be null or a non-empty value"));
        assert!(message.contains("unsupported frontmatter field `published`"));
    }

    #[test]
    fn splits_frontmatter_and_body_without_rewriting_markdown() {
        let path = Path::new("articles/example/article.md");
        let content = "---\ntitle: Example\ntags: []\nauthor: codex\nqiita_id: null\n---\n# Body\n";

        let (frontmatter, body) =
            split_frontmatter(path, content).expect("frontmatter should split");

        assert!(frontmatter.contains("title: Example"));
        assert_eq!(body, "# Body\n");
    }
}
