use crate::application::check::ArticleRepository;
use std::fs;
use std::path::{Path, PathBuf};

pub struct FileSystemArticleRepository;

impl ArticleRepository for FileSystemArticleRepository {
    fn discover_article_paths(&self, root: &Path) -> Result<Vec<PathBuf>, String> {
        let articles_dir = root.join("articles");

        if !articles_dir.exists() {
            return Ok(Vec::new());
        }

        if !articles_dir.is_dir() {
            return Err("articles path exists but is not a directory".to_owned());
        }

        let entries = fs::read_dir(&articles_dir).map_err(|error| {
            format!(
                "failed to read articles directory `{}`: {error}",
                articles_dir.display()
            )
        })?;

        let mut paths = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|error| format!("failed to read article entry: {error}"))?;
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

    fn read_article(&self, path: &Path) -> Result<String, String> {
        fs::read_to_string(path)
            .map_err(|error| format!("{}: failed to read article: {error}", path.display()))
    }
}
