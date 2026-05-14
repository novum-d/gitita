use reqwest::header::{ACCEPT, AUTHORIZATION};
use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct QiitaConfig {
    token: String,
    api_base_url: String,
    upload_policies_url: String,
    timeout: Duration,
}

impl QiitaConfig {
    pub fn from_env() -> Result<Self, QiitaError> {
        dotenvy::dotenv().ok();
        let token = env::var("QIITA_TOKEN").map_err(|_| QiitaError::MissingToken)?;
        let api_base_url = env::var("API_BASE_URL").map_err(|_| QiitaError::MissingBaseUrl)?;
        let upload_policies_url =
            env::var("UPLOAD_POLICIES_URL").map_err(|_| QiitaError::MissingUploadPoliciesUrl)?;
        Self::new(token, api_base_url, upload_policies_url)
    }

    pub fn new(
        token: impl Into<String>,
        api_base_url: impl Into<String>,
        upload_policies_url: impl Into<String>,
    ) -> Result<Self, QiitaError> {
        let token = token.into();
        let api_base_url = api_base_url.into();
        let upload_policies_url = upload_policies_url.into();

        if token.trim().is_empty() {
            return Err(QiitaError::MissingToken);
        }
        if api_base_url.trim().is_empty() {
            return Err(QiitaError::MissingBaseUrl);
        }
        if upload_policies_url.trim().is_empty() {
            return Err(QiitaError::MissingUploadPoliciesUrl);
        }

        Ok(Self {
            token,
            api_base_url,
            upload_policies_url,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    pub fn with_api_base_url(mut self, api_base_url: impl Into<String>) -> Self {
        self.api_base_url = api_base_url
            .into()
            .trim_end_matches('/') // 文字連結時の二重バックスラッシュ防止
            .to_owned();
        self
    }

    pub fn with_upload_policies_url(mut self, upload_policies_url: impl Into<String>) -> Self {
        self.upload_policies_url = upload_policies_url.into();
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

impl fmt::Debug for QiitaConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QiitaConfig")
            .field("token", &"***") // ログ出力時に生のトークンを隠す
            .field("api_base_url", &self.api_base_url)
            .field("upload_policies_url", &self.upload_policies_url)
            .field("timeout", &self.timeout)
            .finish()
    }
}

#[derive(Clone)]
pub struct QiitaClient {
    http: Client,
    config: QiitaConfig,
}

impl QiitaClient {
    pub fn from_env() -> Result<Self, QiitaError> {
        Self::new(QiitaConfig::from_env()?)
    }

    pub fn new(config: QiitaConfig) -> Result<Self, QiitaError> {
        let http = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(QiitaError::HttpClient)?;

        Ok(Self { http, config })
    }

    pub async fn upload_image(&self, image_path: &Path) -> Result<UploadedImage, QiitaError> {
        let file_name = image_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .ok_or_else(|| QiitaError::InvalidImagePath(image_path.display().to_string()))?
            .to_owned();
        let bytes = std::fs::read(image_path).map_err(|error| QiitaError::ReadImage {
            path: image_path.display().to_string(),
            source: error,
        })?;
        let content_type = mime_guess::from_path(image_path)
            .first_or_octet_stream()
            .to_string();

        // Qiita APIからS3署名済みポリシーを取得
        let policy_request = UploadPolicyRequest {
            image: ImageMeta {
                size: bytes.len() as u64,
                content_type: content_type.clone(),
                name: file_name.clone(),
            },
        };
        let policy_response = self
            .http
            .request(Method::POST, &self.config.upload_policies_url)
            .header(AUTHORIZATION, self.authorization_header())
            .header(ACCEPT, "application/json")
            .json(&policy_request)
            .send()
            .await
            .map_err(QiitaError::Request)?;
        let policy = self
            .parse_response::<UploadPolicyResponse>(policy_response)
            .await?;

        // S3に直接アップロード
        let part = reqwest::multipart::Part::bytes(bytes).file_name(file_name);
        let form = reqwest::multipart::Form::new()
            .text("key", policy.form.key.clone())
            .text("acl", policy.form.acl)
            .text("Content-Type", policy.form.content_type)
            .text("policy", policy.form.policy)
            .text("x-amz-credential", policy.form.x_amz_credential)
            .text("x-amz-algorithm", policy.form.x_amz_algorithm)
            .text("x-amz-date", policy.form.x_amz_date)
            .text("x-amz-signature", policy.form.x_amz_signature)
            .part("file", part);
        let s3_response = self
            .http
            .post(&policy.upload_url)
            .multipart(form)
            .send()
            .await
            .map_err(QiitaError::Request)?;

        if !s3_response.status().is_success() {
            let status = s3_response.status();
            let text = s3_response.text().await.unwrap_or_default();
            return Err(QiitaError::Api {
                status,
                message: parse_error_message(&text),
            });
        }

        Ok(UploadedImage {
            url: format!("{}/{}", policy.upload_url, policy.form.key),
        })
    }

    pub async fn create_article(
        &self,
        article: &QiitaArticleRequest,
    ) -> Result<QiitaItem, QiitaError> {
        let url = self.items_url();
        let response = self
            .json_request(Method::POST, &url, article)
            .send()
            .await
            .map_err(QiitaError::Request)?;

        self.parse_response(response).await
    }

    pub async fn update_article(
        &self,
        item_id: &str,
        article: &QiitaArticleRequest,
    ) -> Result<QiitaItem, QiitaError> {
        validate_item_id(item_id)?;

        let url = self.item_url(item_id);
        let response = self
            .json_request(Method::PATCH, &url, article)
            .send()
            .await
            .map_err(QiitaError::Request)?;

        self.parse_response(response).await
    }

    fn json_request<T: Serialize + ?Sized>(
        &self,
        method: Method,
        url: &str,
        body: &T,
    ) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header(AUTHORIZATION, self.authorization_header())
            .header(ACCEPT, "application/json")
            .json(body)
    }

    async fn parse_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, QiitaError> {
        let status = response.status();

        if status.is_success() {
            return response.json().await.map_err(QiitaError::ResponseJson);
        }

        let text = response.text().await.unwrap_or_default();
        Err(QiitaError::Api {
            status,
            message: parse_error_message(&text),
        })
    }

    fn items_url(&self) -> String {
        format!("{}/items", self.config.api_base_url)
    }

    fn item_url(&self, item_id: &str) -> String {
        format!("{}/items/{item_id}", self.config.api_base_url)
    }

    fn authorization_header(&self) -> String {
        format!("Bearer {}", self.config.token)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QiitaArticleRequest {
    pub title: String,
    pub body: String,
    pub tags: Vec<QiitaTag>,
    pub private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_url_name: Option<String>,
    pub tweet: bool,
}

impl QiitaArticleRequest {
    pub fn new(
        title: impl Into<String>,
        body: impl Into<String>,
        tags: Vec<QiitaTag>,
        private: bool,
    ) -> Result<Self, QiitaError> {
        let request = Self {
            title: title.into(),
            body: body.into(),
            tags,
            private,
            organization_url_name: env::var("ORGANIZATION_URL_NAME")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            tweet: false,
        };

        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> Result<(), QiitaError> {
        if self.title.trim().is_empty() {
            return Err(QiitaError::InvalidArticle("title must not be empty"));
        }

        if self.body.trim().is_empty() {
            return Err(QiitaError::InvalidArticle("body must not be empty"));
        }

        if self.tags.is_empty() {
            return Err(QiitaError::InvalidArticle("tags must not be empty"));
        }

        if self.tags.iter().any(|tag| tag.name.trim().is_empty()) {
            return Err(QiitaError::InvalidArticle("tag names must not be empty"));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QiitaTag {
    pub name: String,
    pub versions: Vec<String>,
}

impl QiitaTag {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            versions: Vec::new(),
        }
    }

    pub fn with_versions(name: impl Into<String>, versions: Vec<String>) -> Self {
        Self {
            name: name.into(),
            versions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct QiitaItem {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedImage {
    pub url: String,
}

#[derive(Debug, Serialize)]
struct UploadPolicyRequest {
    image: ImageMeta,
}

#[derive(Debug, Serialize)]
struct ImageMeta {
    size: u64,
    content_type: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct UploadPolicyResponse {
    upload_url: String,
    form: UploadPolicyForm,
}

#[derive(Debug, Deserialize)]
struct UploadPolicyForm {
    key: String,
    acl: String,
    policy: String,
    #[serde(rename = "Content-Type")]
    content_type: String,
    #[serde(rename = "x-amz-credential")]
    x_amz_credential: String,
    #[serde(rename = "x-amz-algorithm")]
    x_amz_algorithm: String,
    #[serde(rename = "x-amz-date")]
    x_amz_date: String,
    #[serde(rename = "x-amz-signature")]
    x_amz_signature: String,
}

#[derive(Debug)]
pub enum QiitaError {
    MissingToken,
    MissingBaseUrl,
    MissingUploadPoliciesUrl,
    InvalidArticle(&'static str),
    InvalidItemId(String),
    InvalidImagePath(String),
    ReadImage {
        path: String,
        source: std::io::Error,
    },
    HttpClient(reqwest::Error),
    Request(reqwest::Error),
    ResponseJson(reqwest::Error),
    ResponseMissingField(&'static str),
    Api {
        status: StatusCode,
        message: String,
    },
}

impl fmt::Display for QiitaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingToken => write!(f, "QIITA_TOKEN environment variable is required"),
            Self::MissingBaseUrl => write!(f, "API_BASE_URL environment variable is required"),
            Self::MissingUploadPoliciesUrl => {
                write!(f, "UPLOAD_POLICIES_URL environment variable is required")
            }
            Self::InvalidArticle(message) => write!(f, "invalid Qiita article request: {message}"),
            Self::InvalidItemId(item_id) => write!(f, "invalid Qiita item id: `{item_id}`"),
            Self::InvalidImagePath(path) => write!(f, "invalid image path: `{path}`"),
            Self::ReadImage { path, source } => {
                write!(f, "failed to read image `{path}`: {source}")
            }
            Self::HttpClient(error) => write!(f, "failed to build Qiita HTTP client: {error}"),
            Self::Request(error) => write!(f, "Qiita API request failed: {error}"),
            Self::ResponseJson(error) => write!(f, "failed to parse Qiita API response: {error}"),
            Self::ResponseMissingField(field) => {
                write!(f, "Qiita API response missing required field `{field}`")
            }
            Self::Api { status, message } => {
                write!(f, "Qiita API returned {status}: {message}")
            }
        }
    }
}

impl Error for QiitaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadImage { source, .. } => Some(source),
            Self::HttpClient(error) | Self::Request(error) | Self::ResponseJson(error) => {
                Some(error)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    message: Option<String>,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

fn parse_error_message(text: &str) -> String {
    if let Ok(response) = serde_json::from_str::<ErrorResponse>(text) {
        if let Some(message) = response.message {
            return message;
        }

        if let Some(error_type) = response.error_type {
            return error_type;
        }
    }

    if text.trim().is_empty() {
        "empty error response".to_owned()
    } else {
        text.trim().to_owned()
    }
}

fn validate_item_id(item_id: &str) -> Result<(), QiitaError> {
    let valid = !item_id.trim().is_empty()
        && item_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');

    if valid {
        Ok(())
    } else {
        Err(QiitaError::InvalidItemId(item_id.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_error_message, validate_item_id, QiitaArticleRequest, QiitaConfig, QiitaError,
        QiitaTag,
    };
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn article_request_serializes_for_qiita_item_api() {
        let request = QiitaArticleRequest::new(
            "Rust CLI",
            "# Body",
            vec![QiitaTag::with_versions("Rust", vec!["1.80".to_owned()])],
            true,
        )
        .expect("valid request should be accepted");

        let value = serde_json::to_value(request).expect("request should serialize");

        assert_eq!(
            value,
            json!({
                "title": "Rust CLI",
                "body": "# Body",
                "tags": [
                    {
                        "name": "Rust",
                        "versions": ["1.80"]
                    }
                ],
                "private": true,
                "tweet": false
            })
        );
    }

    #[test]
    fn article_request_rejects_empty_required_fields() {
        let error =
            QiitaArticleRequest::new("", "body", vec![QiitaTag::new("Rust")], true).unwrap_err();

        assert_eq!(
            error.to_string(),
            "invalid Qiita article request: title must not be empty"
        );
    }

    #[test]
    fn config_requires_non_empty_token() {
        let error = QiitaConfig::new(
            " ",
            "https://qiita.com/api/v2",
            "https://qiita.com/api/upload/policies",
        )
        .expect_err("empty token should fail");

        assert_eq!(
            error.to_string(),
            "QIITA_TOKEN environment variable is required"
        );
    }

    #[test]
    fn config_uses_explicit_default_timeout() {
        let config = QiitaConfig::new(
            "secret",
            "https://qiita.com/api/v2",
            "https://qiita.com/api/upload/policies",
        )
        .expect("token should be accepted")
        .with_timeout(Duration::from_secs(12));

        assert_eq!(config.timeout(), Duration::from_secs(12));
    }

    #[test]
    fn config_debug_redacts_token() {
        let config = QiitaConfig::new(
            "super-secret-token",
            "https://qiita.com/api/v2",
            "https://qiita.com/api/upload/policies",
        )
        .expect("token should be accepted");
        let debug = format!("{config:?}");

        assert!(debug.contains("***"));
        assert!(!debug.contains("super-secret-token"));
    }

    #[test]
    fn validates_item_id_before_building_update_url() {
        validate_item_id("abc123-def").expect("safe item id should be accepted");

        let error = validate_item_id("../secret").expect_err("unsafe id should fail");

        assert!(matches!(error, QiitaError::InvalidItemId(item_id) if item_id == "../secret"));
    }

    #[test]
    fn parses_readable_api_error_message() {
        let message = parse_error_message(r#"{"message":"Unauthorized","type":"unauthorized"}"#);

        assert_eq!(message, "Unauthorized");
    }
}
