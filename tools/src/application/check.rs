use crate::domain::frontmatter::{parse_frontmatter, validate_frontmatter};
use std::path::{Path, PathBuf};

pub trait ArticleRepository {
    fn discover_article_paths(&self, root: &Path) -> Result<Vec<PathBuf>, String>;
    fn read_article(&self, path: &Path) -> Result<String, String>;
}

pub fn check_articles(repository: &impl ArticleRepository, root: &Path) -> Result<(), String> {
    validate_articles(repository, root)
}

pub fn validate_articles(repository: &impl ArticleRepository, root: &Path) -> Result<(), String> {
    let article_paths = repository.discover_article_paths(root)?;
    let mut errors = Vec::new();

    for path in article_paths {
        if let Err(error) = validate_article(repository, &path) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn validate_article(repository: &impl ArticleRepository, path: &Path) -> Result<(), String> {
    let content = repository.read_article(path)?;
    let frontmatter =
        parse_frontmatter(&content).map_err(|error| format!("{}: {error}", path.display()))?;
    let violations = validate_frontmatter(&frontmatter);

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations
            .into_iter()
            .map(|violation| format!("{}: {violation}", path.display()))
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_articles, ArticleRepository};
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    #[derive(Default)]
    struct MemoryArticleRepository {
        articles: BTreeMap<PathBuf, String>,
    }

    impl ArticleRepository for MemoryArticleRepository {
        fn discover_article_paths(&self, _root: &Path) -> Result<Vec<PathBuf>, String> {
            Ok(self.articles.keys().cloned().collect())
        }

        fn read_article(&self, path: &Path) -> Result<String, String> {
            self.articles
                .get(path)
                .cloned()
                .ok_or_else(|| format!("{}: failed to read article", path.display()))
        }
    }

    #[test]
    fn validates_articles_without_filesystem_io() {
        let mut repository = MemoryArticleRepository::default();
        repository.articles.insert(
            PathBuf::from("articles/example/article.md"),
            r#"---
title: "Valid Article"
tags:
  - rust
author: "codex"
qiita_id: null
---

# Valid Article
"#
            .to_owned(),
        );

        validate_articles(&repository, Path::new(".")).expect("article should be valid");
    }

    #[test]
    fn includes_article_path_in_validation_errors() {
        let mut repository = MemoryArticleRepository::default();
        repository.articles.insert(
            PathBuf::from("articles/example/article.md"),
            r#"---
tags:
  - rust
author: "codex"
qiita_id: null
---
"#
            .to_owned(),
        );

        let error =
            validate_articles(&repository, Path::new(".")).expect_err("article should be invalid");

        assert_eq!(
            error,
            "articles/example/article.md: missing required frontmatter field `title`"
        );
    }
}
