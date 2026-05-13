use crate::diff::{self, ChangedArticle};
use crate::markdown::{self, ResolvedImageReference};
use crate::qiita::{
    QiitaArticleRequest, QiitaClient, QiitaError, QiitaItem, QiitaTag, UploadedImage,
};
use crate::{parse_frontmatter, split_frontmatter, validate_article};
use serde_yaml::Value;
use std::error::Error;
use std::fmt;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishOptions {
    pub dry_run: bool,
    pub diff_base: String,
    pub diff_head: String,
}

impl PublishOptions {
    pub fn from_env(dry_run: bool) -> Self {
        Self {
            dry_run,
            diff_base: std::env::var("GITITA_DIFF_BASE").unwrap_or_else(|_| "HEAD^".to_owned()),
            diff_head: std::env::var("GITITA_DIFF_HEAD").unwrap_or_else(|_| "HEAD".to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishReport {
    pub targets: Vec<PublishTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishTarget {
    pub slug: String,
    pub action: PublishAction,
    pub upload_targets: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishAction {
    Create,
    Update { qiita_id: String },
}

#[derive(Debug)]
pub struct PreparedArticle {
    pub slug: String,
    pub path: PathBuf,
    pub title: String,
    pub tags: Vec<QiitaTag>,
    pub qiita_id: Option<String>,
    pub body: String,
    pub images: Vec<ResolvedImageReference>,
}

pub trait QiitaPublisher {
    fn upload_image<'a>(
        &'a self,
        image_path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<UploadedImage, QiitaError>> + 'a>>;

    fn create_article<'a>(
        &'a self,
        article: &'a QiitaArticleRequest,
    ) -> Pin<Box<dyn Future<Output = Result<QiitaItem, QiitaError>> + 'a>>;

    fn update_article<'a>(
        &'a self,
        item_id: &'a str,
        article: &'a QiitaArticleRequest,
    ) -> Pin<Box<dyn Future<Output = Result<QiitaItem, QiitaError>> + 'a>>;
}

impl QiitaPublisher for QiitaClient {
    fn upload_image<'a>(
        &'a self,
        image_path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<UploadedImage, QiitaError>> + 'a>> {
        Box::pin(async move { self.upload_image(image_path).await })
    }

    fn create_article<'a>(
        &'a self,
        article: &'a QiitaArticleRequest,
    ) -> Pin<Box<dyn Future<Output = Result<QiitaItem, QiitaError>> + 'a>> {
        Box::pin(async move { self.create_article(article).await })
    }

    fn update_article<'a>(
        &'a self,
        item_id: &'a str,
        article: &'a QiitaArticleRequest,
    ) -> Pin<Box<dyn Future<Output = Result<QiitaItem, QiitaError>> + 'a>> {
        Box::pin(async move { self.update_article(item_id, article).await })
    }
}

#[derive(Debug)]
pub enum WorkflowError {
    Diff(diff::DiffError),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Validation(String),
    ImageValidation(String),
    InvalidFrontmatter {
        path: PathBuf,
        message: String,
    },
    Qiita(QiitaError),
}

impl fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diff(error) => write!(f, "{error}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Validation(message) => f.write_str(message),
            Self::ImageValidation(message) => f.write_str(message),
            Self::InvalidFrontmatter { path, message } => {
                write!(f, "{}: {message}", path.display())
            }
            Self::Qiita(error) => write!(f, "{error}"),
        }
    }
}

impl Error for WorkflowError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Diff(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Qiita(error) => Some(error),
            _ => None,
        }
    }
}

impl From<diff::DiffError> for WorkflowError {
    fn from(error: diff::DiffError) -> Self {
        Self::Diff(error)
    }
}

impl From<QiitaError> for WorkflowError {
    fn from(error: QiitaError) -> Self {
        Self::Qiita(error)
    }
}

pub async fn publish(root: &Path, options: PublishOptions) -> Result<(), WorkflowError> {
    let changed_articles = diff::detect_changed_articles(&options.diff_base, &options.diff_head)?;

    if options.dry_run {
        let report = dry_run_report(root, &changed_articles)?;
        print_dry_run_report(&report);
        return Ok(());
    }

    let client = QiitaClient::from_env()?;
    publish_changed_articles(root, &changed_articles, &client).await
}

pub fn dry_run_report(
    root: &Path,
    changed_articles: &[ChangedArticle],
) -> Result<PublishReport, WorkflowError> {
    let mut targets = Vec::new();

    for changed_article in changed_articles {
        let article = prepare_article(root, &changed_article.slug)?;

        targets.push(PublishTarget {
            slug: article.slug,
            action: publish_action(article.qiita_id.as_deref()),
            upload_targets: article
                .images
                .into_iter()
                .map(|image| image.image_path)
                .collect(),
        });
    }

    Ok(PublishReport { targets })
}

pub async fn publish_changed_articles<P: QiitaPublisher>(
    root: &Path,
    changed_articles: &[ChangedArticle],
    publisher: &P,
) -> Result<(), WorkflowError> {
    for changed_article in changed_articles {
        let article = prepare_article(root, &changed_article.slug)?;
        publish_prepared_article(article, publisher).await?;
    }

    Ok(())
}

pub fn prepare_article(root: &Path, slug: &str) -> Result<PreparedArticle, WorkflowError> {
    let path = root.join("articles").join(slug).join("article.md");

    validate_article(&path).map_err(WorkflowError::Validation)?;

    let content = fs::read_to_string(&path).map_err(|source| WorkflowError::Io {
        path: path.clone(),
        source,
    })?;
    let frontmatter = parse_frontmatter(&path, &content).map_err(WorkflowError::Validation)?;
    let (_, body) = split_frontmatter(&path, &content).map_err(WorkflowError::Validation)?;
    let images = markdown::resolve_image_references(&path, body).map_err(|errors| {
        WorkflowError::ImageValidation(
            errors
                .into_iter()
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })?;

    Ok(PreparedArticle {
        slug: slug.to_owned(),
        path: path.clone(),
        title: required_string(&path, "title", frontmatter.title)?,
        tags: qiita_tags(&path, frontmatter.tags)?,
        qiita_id: qiita_id(&path, frontmatter.qiita_id)?,
        body: body.to_owned(),
        images,
    })
}

pub fn publish_action(qiita_id: Option<&str>) -> PublishAction {
    match qiita_id {
        Some(qiita_id) => PublishAction::Update {
            qiita_id: qiita_id.to_owned(),
        },
        None => PublishAction::Create,
    }
}

async fn publish_prepared_article<P: QiitaPublisher>(
    article: PreparedArticle,
    publisher: &P,
) -> Result<(), WorkflowError> {
    let mut replacements = Vec::new();

    for image in &article.images {
        let uploaded = publisher.upload_image(&image.image_path).await?;
        replacements.push((image.source.clone(), uploaded.url));
    }

    let body = markdown::replace_image_sources(&article.body, &replacements);
    let request = QiitaArticleRequest::new(article.title, body, article.tags)?;

    match publish_action(article.qiita_id.as_deref()) {
        PublishAction::Create => {
            let item = publisher.create_article(&request).await?;
            update_qiita_id(&article.path, &item.id)?;
        }
        PublishAction::Update { qiita_id } => {
            publisher.update_article(&qiita_id, &request).await?;
        }
    }

    Ok(())
}

fn required_string(
    path: &Path,
    field: &'static str,
    value: Option<String>,
) -> Result<String, WorkflowError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| WorkflowError::InvalidFrontmatter {
            path: path.to_path_buf(),
            message: format!("frontmatter field `{field}` must not be empty"),
        })
}

fn qiita_id(path: &Path, value: Option<Value>) -> Result<Option<String>, WorkflowError> {
    match value {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value)),
        Some(_) => Err(WorkflowError::InvalidFrontmatter {
            path: path.to_path_buf(),
            message: "frontmatter field `qiita_id` must be null or a non-empty string".to_owned(),
        }),
        None => Err(WorkflowError::InvalidFrontmatter {
            path: path.to_path_buf(),
            message: "missing required frontmatter field `qiita_id`".to_owned(),
        }),
    }
}

fn qiita_tags(path: &Path, values: Option<Vec<Value>>) -> Result<Vec<QiitaTag>, WorkflowError> {
    let values = values.ok_or_else(|| WorkflowError::InvalidFrontmatter {
        path: path.to_path_buf(),
        message: "missing required frontmatter field `tags`".to_owned(),
    })?;

    values
        .into_iter()
        .map(|value| qiita_tag(path, value))
        .collect()
}

fn qiita_tag(path: &Path, value: Value) -> Result<QiitaTag, WorkflowError> {
    match value {
        Value::String(name) if !name.trim().is_empty() => Ok(QiitaTag::new(name)),
        Value::Mapping(mapping) => {
            let name = mapping
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .ok_or_else(|| WorkflowError::InvalidFrontmatter {
                    path: path.to_path_buf(),
                    message: "tag mapping must include a non-empty `name`".to_owned(),
                })?;
            let versions = mapping
                .get("versions")
                .and_then(Value::as_sequence)
                .map(|versions| {
                    versions
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect()
                })
                .unwrap_or_default();

            Ok(QiitaTag::with_versions(name, versions))
        }
        _ => Err(WorkflowError::InvalidFrontmatter {
            path: path.to_path_buf(),
            message: "tags must be strings or mappings with `name`".to_owned(),
        }),
    }
}

fn update_qiita_id(path: &Path, qiita_id: &str) -> Result<(), WorkflowError> {
    let content = fs::read_to_string(path).map_err(|source| WorkflowError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let updated = replace_qiita_id_in_frontmatter(path, &content, qiita_id)?;

    fs::write(path, updated).map_err(|source| WorkflowError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn replace_qiita_id_in_frontmatter(
    path: &Path,
    content: &str,
    qiita_id: &str,
) -> Result<String, WorkflowError> {
    let mut output = String::with_capacity(content.len() + qiita_id.len());
    let mut in_frontmatter = false;
    let mut replaced = false;

    for (index, line) in content.split_inclusive('\n').enumerate() {
        let content_line = line
            .strip_suffix("\r\n")
            .or_else(|| line.strip_suffix('\n'))
            .unwrap_or(line);
        let ending = &line[content_line.len()..];

        if index == 0 && content_line == "---" {
            in_frontmatter = true;
            output.push_str(line);
            continue;
        }

        if in_frontmatter && content_line == "---" {
            in_frontmatter = false;
            output.push_str(line);
            continue;
        }

        if in_frontmatter && content_line.trim_start().starts_with("qiita_id:") {
            let indent_len = content_line.len() - content_line.trim_start().len();
            output.push_str(&content_line[..indent_len]);
            output.push_str("qiita_id: \"");
            output.push_str(&qiita_id.replace('"', "\\\""));
            output.push('"');
            output.push_str(ending);
            replaced = true;
        } else {
            output.push_str(line);
        }
    }

    if replaced {
        Ok(output)
    } else {
        Err(WorkflowError::InvalidFrontmatter {
            path: path.to_path_buf(),
            message: "missing required frontmatter field `qiita_id`".to_owned(),
        })
    }
}

fn print_dry_run_report(report: &PublishReport) {
    if report.targets.is_empty() {
        println!("No changed articles to publish.");
        return;
    }

    for target in &report.targets {
        match &target.action {
            PublishAction::Create => {
                println!("publish target: {} create", target.slug);
            }
            PublishAction::Update { qiita_id } => {
                println!("publish target: {} update {qiita_id}", target.slug);
            }
        }

        for upload_target in &target.upload_targets {
            println!("upload target: {}", upload_target.display());
        }
    }

    println!("dry-run: skipped Qiita API calls");
}

#[cfg(test)]
mod tests {
    use super::{
        dry_run_report, prepare_article, publish_action, replace_qiita_id_in_frontmatter,
        PublishAction,
    };
    use crate::diff::ChangedArticle;
    use std::path::Path;

    #[test]
    fn dry_run_reports_publish_targets_without_api_client() {
        let report = dry_run_report(
            Path::new("tests/fixtures/markdown-valid"),
            &[ChangedArticle {
                slug: "markdown-images".to_owned(),
                is_new: true,
            }],
        )
        .expect("dry-run should prepare report");

        assert_eq!(report.targets.len(), 1);
        assert_eq!(report.targets[0].slug, "markdown-images");
        assert_eq!(report.targets[0].action, PublishAction::Create);
        assert_eq!(report.targets[0].upload_targets.len(), 2);
    }

    #[test]
    fn selects_create_or_update_from_qiita_id() {
        assert_eq!(publish_action(None), PublishAction::Create);
        assert_eq!(
            publish_action(Some("abc123")),
            PublishAction::Update {
                qiita_id: "abc123".to_owned()
            }
        );
    }

    #[test]
    fn prepares_article_with_create_action_when_qiita_id_is_null() {
        let article = prepare_article(
            Path::new("tests/fixtures/frontmatter-valid"),
            "valid-article",
        )
        .expect("valid article should prepare");

        assert!(article.qiita_id.is_none());
        assert_eq!(
            publish_action(article.qiita_id.as_deref()),
            PublishAction::Create
        );
    }

    #[test]
    fn prepares_article_with_update_action_when_qiita_id_exists() {
        let article = prepare_article(
            Path::new("tests/fixtures/publish-update"),
            "published-article",
        )
        .expect("published article should prepare");

        assert_eq!(
            publish_action(article.qiita_id.as_deref()),
            PublishAction::Update {
                qiita_id: "abc123".to_owned()
            }
        );
    }

    #[test]
    fn replaces_only_qiita_id_in_frontmatter() {
        let content = "---\ntitle: Example\ntags: []\nauthor: codex\nqiita_id: null\n---\n# Body\n![img](./images/example.png)\n";

        let updated = replace_qiita_id_in_frontmatter(
            Path::new("articles/example/article.md"),
            content,
            "abc123",
        )
        .expect("qiita_id should update");

        assert!(updated.contains("qiita_id: \"abc123\""));
        assert!(updated.contains("# Body\n![img](./images/example.png)\n"));
    }
}
