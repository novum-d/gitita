use gitita::diff::ChangedArticle;
use gitita::qiita::{QiitaClient, QiitaConfig};
use gitita::workflow::publish_changed_articles;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, NamedTempFile};

#[derive(Debug, Deserialize)]
struct RemoteTag {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RemoteUser {
    id: String,
}

#[derive(Debug, Deserialize)]
struct RemoteItem {
    id: String,
    url: String,
    title: String,
    body: String,
    tags: Vec<RemoteTag>,
    user: RemoteUser,
}

#[tokio::test]
#[ignore = "requires real Qiita credentials and network access"]
async fn publish_create_and_update_matches_remote_item() {
    dotenvy::dotenv().ok();

    let token = env::var("QIITA_TOKEN").expect("QIITA_TOKEN must be set");
    let api_base_url = env::var("API_BASE_URL").expect("API_BASE_URL must be set");
    let upload_policies_url =
        env::var("UPLOAD_POLICIES_URL").expect("UPLOAD_POLICIES_URL must be set");
    let expected_author = env::var("QIITA_USER_ID").expect("QIITA_USER_ID must be set");

    let root_dir = tempdir().expect("create temp dir");
    let root = root_dir.path();
    let slug = "real-publish-e2e";
    let article_dir = root.join("articles").join(slug);
    let images_dir = article_dir.join("images");
    fs::create_dir_all(&images_dir).expect("create article directories");

    let image_path = images_dir.join("image_upload_test.png");
    fs::copy("tests/fixtures/image_upload_test.png", &image_path).expect("copy image fixture");
    let local_image_bytes = fs::read(&image_path).expect("read local image");

    let title_create = format!("Real Publish E2E Create {}", chrono_like_timestamp());
    let title_update = format!("Real Publish E2E Update {}", chrono_like_timestamp());
    let tags_create = vec!["rust", "gitita-e2e-create"];
    let tags_update = vec!["rust", "gitita-e2e-update"];
    let body_create = "![img](./images/image_upload_test.png)\n\ncreate-phase";
    let body_update = "![img](./images/image_upload_test.png)\n\nupdate-phase";

    write_article(
        &article_dir.join("article.md"),
        &title_create,
        &tags_create,
        &expected_author,
        None,
        body_create,
    );

    let config = QiitaConfig::new(token.clone(), api_base_url.clone(), upload_policies_url)
        .expect("build qiita config");
    let publisher = QiitaClient::new(config).expect("build qiita client");

    let mut created_qiita_id_for_cleanup: Option<String> = None;
    let test_result: Result<(), String> = async {
        publish_changed_articles(
            root,
            &[ChangedArticle {
                slug: slug.to_owned(),
                is_new: true,
            }],
            &publisher,
        )
        .await
        .map_err(|e| format!("publish create failed: {e}"))?;

        let created_qiita_id = read_qiita_id(&article_dir.join("article.md"));
        created_qiita_id_for_cleanup = Some(created_qiita_id.clone());
        let created_remote = fetch_item(&token, &api_base_url, &created_qiita_id).await?;
        println!("created_item_url={}", created_remote.url);
        if created_remote.id != created_qiita_id {
            return Err("created item id mismatch".to_owned());
        }
        if created_remote.title != title_create {
            return Err("created title mismatch".to_owned());
        }
        if !created_remote.body.contains("create-phase") {
            return Err("created body mismatch".to_owned());
        }
        if normalize_tags(extract_tag_names(&created_remote.tags))
            != normalize_tags(tags_create.clone())
        {
            return Err("created tags mismatch".to_owned());
        }
        if created_remote.user.id != expected_author {
            return Err("created author mismatch".to_owned());
        }
        let created_image_url = extract_uploaded_image_url(&created_remote.body);
        let created_image_bytes = download_image_to_tempfile(&created_image_url).await?;
        if created_image_bytes != local_image_bytes {
            return Err("created image bytes mismatch".to_owned());
        }

        write_article(
            &article_dir.join("article.md"),
            &title_update,
            &tags_update,
            &expected_author,
            Some(&created_qiita_id),
            body_update,
        );

        publish_changed_articles(
            root,
            &[ChangedArticle {
                slug: slug.to_owned(),
                is_new: false,
            }],
            &publisher,
        )
        .await
        .map_err(|e| format!("publish update failed: {e}"))?;

        let updated_qiita_id = read_qiita_id(&article_dir.join("article.md"));
        let updated_remote = fetch_item(&token, &api_base_url, &updated_qiita_id).await?;
        if updated_remote.id != created_qiita_id {
            return Err("updated item id mismatch".to_owned());
        }
        if updated_remote.title != title_update {
            return Err("updated title mismatch".to_owned());
        }
        if !updated_remote.body.contains("update-phase") {
            return Err("updated body mismatch".to_owned());
        }
        if normalize_tags(extract_tag_names(&updated_remote.tags))
            != normalize_tags(tags_update.clone())
        {
            return Err("updated tags mismatch".to_owned());
        }
        if updated_remote.user.id != expected_author {
            return Err("updated author mismatch".to_owned());
        }
        let updated_image_url = extract_uploaded_image_url(&updated_remote.body);
        let updated_image_bytes = download_image_to_tempfile(&updated_image_url).await?;
        if updated_image_bytes != local_image_bytes {
            return Err("updated image bytes mismatch".to_owned());
        }
        Ok(())
    }
    .await;

    if let Some(item_id) = &created_qiita_id_for_cleanup {
        let _ = delete_item(&token, &api_base_url, item_id).await;
    }

    if let Err(error) = test_result {
        panic!("{error}");
    }
}

fn write_article(
    path: &Path,
    title: &str,
    tags: &[&str],
    author: &str,
    qiita_id: Option<&str>,
    body: &str,
) {
    let tags_yaml = tags
        .iter()
        .map(|tag| format!("  - {tag}"))
        .collect::<Vec<_>>()
        .join("\n");
    let qiita_id_yaml = qiita_id
        .map(|id| format!("\"{id}\""))
        .unwrap_or_else(|| "null".to_owned());
    let content = format!(
        "---\ntitle: \"{title}\"\ntags:\n{tags_yaml}\nauthor: \"{author}\"\nqiita_id: {qiita_id_yaml}\n---\n\n{body}\n"
    );
    fs::write(path, content).expect("write article");
}

fn read_qiita_id(path: &Path) -> String {
    let content = fs::read_to_string(path).expect("read article");
    for line in content.lines() {
        if line.trim_start().starts_with("qiita_id:") {
            return line
                .split_once(':')
                .map(|(_, right)| right.trim().trim_matches('"').to_owned())
                .filter(|value| !value.is_empty() && value != "null")
                .expect("qiita_id must exist");
        }
    }
    panic!("qiita_id not found");
}

fn extract_tag_names(tags: &[RemoteTag]) -> Vec<&str> {
    tags.iter().map(|tag| tag.name.as_str()).collect()
}

fn normalize_tags<T: AsRef<str>>(tags: Vec<T>) -> Vec<String> {
    tags.into_iter()
        .map(|tag| tag.as_ref().to_ascii_lowercase())
        .collect()
}

fn extract_uploaded_image_url(body: &str) -> String {
    let marker = "https://";
    let start = body
        .find(marker)
        .expect("remote body must include image url");
    let tail = &body[start..];
    let end = tail.find(')').unwrap_or(tail.len());
    tail[..end].to_owned()
}

async fn fetch_item(token: &str, api_base_url: &str, item_id: &str) -> Result<RemoteItem, String> {
    let url = format!("{}/items/{item_id}", api_base_url.trim_end_matches('/'));
    let response = reqwest::Client::new()
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("failed to fetch item: {status} {body}"));
    }
    response
        .json::<RemoteItem>()
        .await
        .map_err(|e| e.to_string())
}

async fn delete_item(token: &str, api_base_url: &str, item_id: &str) -> Result<(), String> {
    let url = format!("{}/items/{item_id}", api_base_url.trim_end_matches('/'));
    let response = reqwest::Client::new()
        .delete(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.status().is_success() {
        return Ok(());
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    Err(format!("failed to delete item: {status} {body}"))
}

async fn download_image_to_tempfile(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("failed to download image: {}", response.status()));
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let file = NamedTempFile::new().map_err(|e| e.to_string())?;
    let path: PathBuf = file.path().to_path_buf();
    fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    fs::read(path).map_err(|e| e.to_string())
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time must be after epoch");
    format!("{}", now.as_secs())
}
