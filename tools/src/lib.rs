use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

type Frontmatter = BTreeMap<String, FrontmatterValue>;

#[derive(Debug, PartialEq, Eq)]
enum FrontmatterValue {
    Null,
    String(String),
    Array,
}

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

fn validate_article(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("{}: failed to read article: {error}", path.display()))?;
    let frontmatter = parse_frontmatter(path, &content)?;

    validate_required_string(path, &frontmatter, "title")?;
    validate_tags(path, &frontmatter)?;
    validate_required_string(path, &frontmatter, "author")?;
    validate_qiita_id(path, &frontmatter)?;

    Ok(())
}

fn publish_dry_run() -> Result<(), CliError> {
    Ok(())
}

fn parse_frontmatter(path: &Path, content: &str) -> Result<Frontmatter, String> {
    let mut lines = content.lines();

    if lines.next() != Some("---") {
        return Err(format!(
            "{}: missing frontmatter opening delimiter `---`",
            path.display()
        ));
    }

    let mut frontmatter_lines = Vec::new();
    let mut found_closing_delimiter = false;

    for line in lines {
        if line == "---" {
            found_closing_delimiter = true;
            break;
        }

        frontmatter_lines.push(line);
    }

    if !found_closing_delimiter {
        return Err(format!(
            "{}: missing frontmatter closing delimiter `---`",
            path.display()
        ));
    }

    parse_frontmatter_lines(path, &frontmatter_lines)
}

fn validate_required_string(
    path: &Path,
    frontmatter: &Frontmatter,
    field: &str,
) -> Result<(), String> {
    match get_field(frontmatter, field) {
        Some(FrontmatterValue::String(value)) if !value.trim().is_empty() => Ok(()),
        Some(FrontmatterValue::String(_)) => Err(format!(
            "{}: frontmatter field `{field}` must not be empty",
            path.display()
        )),
        Some(_) => Err(format!(
            "{}: frontmatter field `{field}` must be a non-empty string",
            path.display()
        )),
        None => Err(format!(
            "{}: missing required frontmatter field `{field}`",
            path.display()
        )),
    }
}

fn validate_tags(path: &Path, frontmatter: &Frontmatter) -> Result<(), String> {
    match get_field(frontmatter, "tags") {
        Some(FrontmatterValue::Array) => Ok(()),
        Some(_) => Err(format!(
            "{}: frontmatter field `tags` must be an array",
            path.display()
        )),
        None => Err(format!(
            "{}: missing required frontmatter field `tags`",
            path.display()
        )),
    }
}

fn validate_qiita_id(path: &Path, frontmatter: &Frontmatter) -> Result<(), String> {
    if get_field(frontmatter, "published").is_some() {
        return Err(format!(
            "{}: unsupported frontmatter field `published`",
            path.display()
        ));
    }

    match get_field(frontmatter, "qiita_id") {
        Some(FrontmatterValue::Null) => Ok(()),
        Some(FrontmatterValue::String(value)) if !value.trim().is_empty() => Ok(()),
        Some(FrontmatterValue::String(_)) => Err(format!(
            "{}: frontmatter field `qiita_id` must be null or a non-empty value",
            path.display()
        )),
        Some(_) => Ok(()),
        None => Err(format!(
            "{}: missing required frontmatter field `qiita_id`",
            path.display()
        )),
    }
}

fn parse_frontmatter_lines(path: &Path, lines: &[&str]) -> Result<Frontmatter, String> {
    let mut frontmatter = Frontmatter::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];

        if line.trim().is_empty() {
            index += 1;
            continue;
        }

        if line.starts_with(char::is_whitespace) {
            return Err(format!(
                "{}: unsupported frontmatter line `{}`",
                path.display(),
                line.trim()
            ));
        }

        let Some((key, raw_value)) = line.split_once(':') else {
            return Err(format!(
                "{}: frontmatter line must contain `key: value`: `{}`",
                path.display(),
                line.trim()
            ));
        };

        let key = key.trim();

        if key.is_empty() {
            return Err(format!(
                "{}: frontmatter key must not be empty",
                path.display()
            ));
        }

        let raw_value = raw_value.trim();

        if raw_value.is_empty() && next_line_starts_array(lines, index + 1) {
            frontmatter.insert(key.to_owned(), FrontmatterValue::Array);

            index += 1;
            while index < lines.len() && lines[index].trim_start().starts_with("- ") {
                index += 1;
            }
            continue;
        }

        frontmatter.insert(key.to_owned(), parse_frontmatter_value(raw_value));
        index += 1;
    }

    Ok(frontmatter)
}

fn next_line_starts_array(lines: &[&str], index: usize) -> bool {
    lines
        .get(index)
        .is_some_and(|line| line.trim_start().starts_with("- "))
}

fn parse_frontmatter_value(value: &str) -> FrontmatterValue {
    match value {
        "null" | "~" => FrontmatterValue::Null,
        value if value.starts_with('[') && value.ends_with(']') => FrontmatterValue::Array,
        value => FrontmatterValue::String(unquote(value).to_owned()),
    }
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn get_field<'a>(frontmatter: &'a Frontmatter, field: &str) -> Option<&'a FrontmatterValue> {
    frontmatter.get(field)
}

#[cfg(test)]
mod tests {
    use super::{parse_command, run, validate_articles, Command};
    use std::path::Path;

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
        // dry-run is an optional flag anywhere after publish
        assert_eq!(
            parse_command(["publish", "something", "--dry-run"]),
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
}
