use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Component, Path};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedArticle {
    pub slug: String,
    pub is_new: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DiffError {
    GitFailed(String),
    InvalidDiffLine(String),
    ForbiddenRename { from: String, to: String },
    MultipleNewArticles { slugs: Vec<String> },
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitFailed(message) => write!(f, "git diff failed: {message}"),
            Self::InvalidDiffLine(line) => {
                write!(f, "failed to parse git diff name-status line: `{line}`")
            }
            Self::ForbiddenRename { from, to } => write!(
                f,
                "article directory rename is forbidden: articles/{from}/ -> articles/{to}/"
            ),
            Self::MultipleNewArticles { slugs } => write!(
                f,
                "multiple new articles are not allowed in one PR: {}",
                slugs.join(", ")
            ),
        }
    }
}

impl Error for DiffError {}

#[derive(Debug, PartialEq, Eq)]
enum DiffEntry {
    Changed {
        status: String,
        path: String,
    },
    Renamed {
        status: String,
        old_path: String,
        new_path: String,
    },
}

/// Detect changed articles using `git diff --name-status --find-renames`.
pub fn detect_changed_articles(base: &str, head: &str) -> Result<Vec<ChangedArticle>, DiffError> {
    let output = Command::new("git")
        .args(["diff", "--name-status", "--find-renames", base, head, "--"])
        .arg("articles")
        .output()
        .map_err(|error| DiffError::GitFailed(error.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("exit status {}", output.status)
        } else {
            stderr
        };

        return Err(DiffError::GitFailed(message));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    changed_articles_from_name_status(&stdout)
}

pub fn changed_articles_from_name_status(
    name_status: &str,
) -> Result<Vec<ChangedArticle>, DiffError> {
    let mut articles = BTreeMap::<String, ChangedArticle>::new();

    for line in name_status.lines().filter(|line| !line.trim().is_empty()) {
        let entry = parse_name_status_line(line)?;

        match entry {
            DiffEntry::Changed { status, path } => {
                if let Some(slug) = article_slug(&path) {
                    let article = articles.entry(slug.clone()).or_insert(ChangedArticle {
                        slug,
                        is_new: false,
                    });

                    if status == "A" && is_article_file(&path) {
                        article.is_new = true;
                    }
                }
            }
            DiffEntry::Renamed {
                status: _,
                old_path,
                new_path,
            } => {
                let old_slug = article_slug(&old_path);
                let new_slug = article_slug(&new_path);

                if let (Some(from), Some(to)) = (&old_slug, &new_slug) {
                    if from != to {
                        return Err(DiffError::ForbiddenRename {
                            from: from.clone(),
                            to: to.clone(),
                        });
                    }
                }

                for slug in old_slug.into_iter().chain(new_slug) {
                    articles.entry(slug.clone()).or_insert(ChangedArticle {
                        slug,
                        is_new: false,
                    });
                }
            }
        }
    }

    let mut new_slugs: Vec<String> = articles
        .values()
        .filter(|article| article.is_new)
        .map(|article| article.slug.clone())
        .collect();

    if new_slugs.len() > 1 {
        new_slugs.sort();
        return Err(DiffError::MultipleNewArticles { slugs: new_slugs });
    }

    Ok(articles.into_values().collect())
}

fn parse_name_status_line(line: &str) -> Result<DiffEntry, DiffError> {
    let parts: Vec<&str> = line.split('\t').collect();

    match parts.as_slice() {
        [status, path] if !status.is_empty() => Ok(DiffEntry::Changed {
            status: (*status).to_owned(),
            path: (*path).to_owned(),
        }),
        [status, old_path, new_path] if status.starts_with('R') && !old_path.is_empty() => {
            Ok(DiffEntry::Renamed {
                status: (*status).to_owned(),
                old_path: (*old_path).to_owned(),
                new_path: (*new_path).to_owned(),
            })
        }
        _ => Err(DiffError::InvalidDiffLine(line.to_owned())),
    }
}

fn article_slug(path: &str) -> Option<String> {
    let mut components = Path::new(path).components();

    if components.next()? != Component::Normal("articles".as_ref()) {
        return None;
    }

    match components.next()? {
        Component::Normal(slug) => Some(slug.to_string_lossy().into_owned()),
        _ => None,
    }
}

fn is_article_file(path: &str) -> bool {
    let mut components = Path::new(path).components();

    matches!(
        (
            components.next(),
            components.next(),
            components.next(),
            components.next()
        ),
        (
            Some(Component::Normal(articles)),
            Some(Component::Normal(_slug)),
            Some(Component::Normal(file_name)),
            None
        ) if articles == OsStr::new("articles") && file_name == OsStr::new("article.md")
    )
}

#[cfg(test)]
mod tests {
    use super::{changed_articles_from_name_status, ChangedArticle, DiffError};

    #[test]
    fn detects_no_article_changes() {
        let articles = changed_articles_from_name_status(
            "M\tREADME.md\nA\tdocs/ai/example.md\nM\tCargo.toml\n",
        )
        .expect("non-article changes should be ignored");

        assert!(articles.is_empty());
    }

    #[test]
    fn detects_one_changed_article() {
        let articles =
            changed_articles_from_name_status("M\tarticles/rust-clap-intro/article.md\n")
                .expect("changed article should be detected");

        assert_eq!(
            articles,
            vec![ChangedArticle {
                slug: "rust-clap-intro".to_owned(),
                is_new: false,
            }]
        );
    }

    #[test]
    fn detects_article_image_changes_by_slug() {
        let articles =
            changed_articles_from_name_status("A\tarticles/rust-clap-intro/images/example.png\n")
                .expect("article image change should be detected");

        assert_eq!(
            articles,
            vec![ChangedArticle {
                slug: "rust-clap-intro".to_owned(),
                is_new: false,
            }]
        );
    }

    #[test]
    fn rejects_multiple_new_articles() {
        let error = changed_articles_from_name_status(
            "A\tarticles/first/article.md\nA\tarticles/second/article.md\n",
        )
        .expect_err("multiple new articles should fail validation");

        assert_eq!(
            error,
            DiffError::MultipleNewArticles {
                slugs: vec!["first".to_owned(), "second".to_owned()],
            }
        );
    }

    #[test]
    fn rejects_forbidden_article_directory_rename() {
        let error = changed_articles_from_name_status(
            "R100\tarticles/old-slug/article.md\tarticles/new-slug/article.md\n",
        )
        .expect_err("article directory rename should fail validation");

        assert_eq!(
            error,
            DiffError::ForbiddenRename {
                from: "old-slug".to_owned(),
                to: "new-slug".to_owned(),
            }
        );
    }

    #[test]
    fn allows_rename_inside_same_article_directory() {
        let articles = changed_articles_from_name_status(
            "R100\tarticles/rust-clap-intro/images/old.png\tarticles/rust-clap-intro/images/new.png\n",
        )
        .expect("same article directory rename should be allowed by diff detector");

        assert_eq!(
            articles,
            vec![ChangedArticle {
                slug: "rust-clap-intro".to_owned(),
                is_new: false,
            }]
        );
    }

    #[test]
    fn records_single_new_article() {
        let articles = changed_articles_from_name_status(
            "A\tarticles/rust-clap-intro/article.md\nA\tarticles/rust-clap-intro/images/example.png\n",
        )
        .expect("one new article should be allowed");

        assert_eq!(
            articles,
            vec![ChangedArticle {
                slug: "rust-clap-intro".to_owned(),
                is_new: true,
            }]
        );
    }
}
