use pulldown_cmark::{Event, Parser, Tag};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "tiff", "avif"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageReference {
    pub kind: ImageReferenceKind,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageReferenceKind {
    Markdown,
    Html,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageValidationError {
    AbsolutePath {
        article_path: PathBuf,
        source: String,
    },
    UnsafePath {
        article_path: PathBuf,
        source: String,
    },
    MissingExtension {
        article_path: PathBuf,
        source: String,
    },
    UnsupportedExtension {
        article_path: PathBuf,
        source: String,
        extension: String,
    },
    MissingFile {
        article_path: PathBuf,
        source: String,
        image_path: PathBuf,
    },
    Symlink {
        article_path: PathBuf,
        source: String,
        image_path: PathBuf,
    },
    NotFile {
        article_path: PathBuf,
        source: String,
        image_path: PathBuf,
    },
}

impl fmt::Display for ImageValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbsolutePath {
                article_path,
                source,
            } => write!(
                f,
                "{}: image path `{source}` must be relative to the article directory",
                article_path.display()
            ),
            Self::UnsafePath {
                article_path,
                source,
            } => write!(
                f,
                "{}: image path `{source}` must stay inside the article directory",
                article_path.display()
            ),
            Self::MissingExtension {
                article_path,
                source,
            } => write!(
                f,
                "{}: image path `{source}` must include a supported image extension",
                article_path.display()
            ),
            Self::UnsupportedExtension {
                article_path,
                source,
                extension,
            } => write!(
                f,
                "{}: unsupported image extension `.{extension}` in `{source}`; supported extensions: {}",
                article_path.display(),
                SUPPORTED_IMAGE_EXTENSIONS.join(", ")
            ),
            Self::MissingFile {
                article_path,
                source,
                image_path,
            } => write!(
                f,
                "{}: image path `{source}` does not exist at `{}`",
                article_path.display(),
                image_path.display()
            ),
            Self::Symlink {
                article_path,
                source,
                image_path,
            } => write!(
                f,
                "{}: image path `{source}` points to a symlink at `{}`",
                article_path.display(),
                image_path.display()
            ),
            Self::NotFile {
                article_path,
                source,
                image_path,
            } => write!(
                f,
                "{}: image path `{source}` must point to a file, got `{}`",
                article_path.display(),
                image_path.display()
            ),
        }
    }
}

impl Error for ImageValidationError {}

pub fn collect_image_references(markdown: &str) -> Vec<ImageReference> {
    let mut references = Vec::new();

    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                let source = dest_url.to_string();

                if is_local_reference(&source) {
                    references.push(ImageReference {
                        kind: ImageReferenceKind::Markdown,
                        source,
                    });
                }
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                references.extend(
                    html_img_sources(&html)
                        .into_iter()
                        .filter(|source| is_local_reference(source))
                        .map(|source| ImageReference {
                            kind: ImageReferenceKind::Html,
                            source,
                        }),
                );
            }
            _ => {}
        }
    }

    references
}

pub fn validate_image_references(
    article_path: &Path,
    markdown: &str,
) -> Result<Vec<ImageReference>, Vec<ImageValidationError>> {
    let references = collect_image_references(markdown);
    let mut errors = Vec::new();

    for reference in &references {
        if let Err(error) = validate_image_reference(article_path, &reference.source) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(references)
    } else {
        Err(errors)
    }
}

fn validate_image_reference(
    article_path: &Path,
    source: &str,
) -> Result<PathBuf, ImageValidationError> {
    let article_dir = article_path.parent().unwrap_or_else(|| Path::new("."));
    let relative_path = Path::new(source);

    if relative_path.is_absolute() {
        return Err(ImageValidationError::AbsolutePath {
            article_path: article_path.to_path_buf(),
            source: source.to_owned(),
        });
    }

    // パスの正規化とサポートされている拡張子の検証
    let normalized_path = normalize_local_path(article_path, source)?;
    validate_supported_extension(article_path, source, &normalized_path)?;

    // シンボリックリンクの検証
    let image_path = article_dir.join(&normalized_path);
    validate_no_symlinked_components(article_path, source, article_dir, &normalized_path)?;

    // メタデータの有無とシンボリックリンクの検証
    let metadata =
        fs::symlink_metadata(&image_path).map_err(|_| ImageValidationError::MissingFile {
            article_path: article_path.to_path_buf(),
            source: source.to_owned(),
            image_path: image_path.clone(),
        })?;

    if metadata.file_type().is_symlink() {
        return Err(ImageValidationError::Symlink {
            article_path: article_path.to_path_buf(),
            source: source.to_owned(),
            image_path,
        });
    }

    if !metadata.file_type().is_file() {
        return Err(ImageValidationError::NotFile {
            article_path: article_path.to_path_buf(),
            source: source.to_owned(),
            image_path,
        });
    }

    Ok(image_path)
}

fn normalize_local_path(
    article_path: &Path,
    source: &str,
) -> Result<PathBuf, ImageValidationError> {
    // 空文字、バックスラッシュ、Windowsのドライブパスは、unsafeパスとして扱う
    if source.trim().is_empty() || source.contains('\\') || is_windows_drive_path(source) {
        return Err(ImageValidationError::UnsafePath {
            article_path: article_path.to_path_buf(),
            source: source.to_owned(),
        });
    }

    let mut path = PathBuf::new();

    for component in Path::new(source).components() {
        match component {
            // 通常のディレクトリ、ファイル名は、ローカルパスとして扱う
            Component::Normal(part) => path.push(part),

            // カレントディレクトリは無視する
            Component::CurDir => {}

            // 絶対パス、親ディレクトリへの参照、プレフィックス（C:やfile:などの特有のドライブパス）はunsafeパスとして扱う
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ImageValidationError::UnsafePath {
                    article_path: article_path.to_path_buf(),
                    source: source.to_owned(),
                });
            }
        }
    }

    // 空のパス（.）はunsafeパスとして扱う
    if path.as_os_str().is_empty() {
        return Err(ImageValidationError::UnsafePath {
            article_path: article_path.to_path_buf(),
            source: source.to_owned(),
        });
    }

    Ok(path)
}

fn validate_supported_extension(
    article_path: &Path,
    source: &str,
    image_path: &Path,
) -> Result<(), ImageValidationError> {
    let extension = image_path
        .extension()
        .and_then(|extension| extension.to_str());

    match extension {
        Some(extension)
        if SUPPORTED_IMAGE_EXTENSIONS
            .iter()
            .any(|supported| extension.eq_ignore_ascii_case(supported)) =>
            {
                Ok(())
            }
        Some(extension) => Err(ImageValidationError::UnsupportedExtension {
            article_path: article_path.to_path_buf(),
            source: source.to_owned(),
            extension: extension.to_ascii_lowercase(),
        }),
        None => Err(ImageValidationError::MissingExtension {
            article_path: article_path.to_path_buf(),
            source: source.to_owned(),
        }),
    }
}

fn validate_no_symlinked_components(
    article_path: &Path,
    source: &str,
    article_dir: &Path,
    image_path: &Path,
) -> Result<(), ImageValidationError> {
    let mut current = article_dir.to_path_buf();

    for component in image_path.components() {
        // 通常のディレクトリ、ファイルパス出ない場合は、スキップ
        let Component::Normal(part) = component else {
            continue;
        };

        current.push(part);

        // メタデータが存在しない場合は、スキップ
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            continue;
        };

        // シンボリックリンクである場合は、シンボリックリンクエラーとして返す
        if metadata.file_type().is_symlink() {
            return Err(ImageValidationError::Symlink {
                article_path: article_path.to_path_buf(),
                source: source.to_owned(),
                image_path: current,
            });
        }
    }

    Ok(())
}

fn is_local_reference(source: &str) -> bool {
    let source = source.trim();
    let lowercase = source.to_ascii_lowercase();

    // 空文字列、アンカー参照（ページ内リンク）、はスキーマなしURL（プロトコル相対URL）は、ローカル参照とみなさない
    if source.is_empty() || source.starts_with('#') || source.starts_with("//") {
        return false;
    }

    // Windowsドライブパスまたはバックスラッシュが含まれている場合は、ローカル参照とする
    if source.contains('\\') || is_windows_drive_path(source) {
        return true;
    }

    // スキーマありのURL（RFC 3986に基づく外部URL）である場合は、ローカル参照とみなさない
    if let Some(colon_index) = lowercase.find(':') {
        let slash_index = lowercase.find('/').unwrap_or(usize::MAX);
        let hash_index = lowercase.find('#').unwrap_or(usize::MAX);
        let query_index = lowercase.find('?').unwrap_or(usize::MAX);

        if colon_index < slash_index && colon_index < hash_index && colon_index < query_index {
            return false;
        }
    }

    true
}

fn is_windows_drive_path(source: &str) -> bool {
    let bytes = source.as_bytes();

    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn html_img_sources(html: &str) -> Vec<String> {
    let bytes = html.as_bytes();
    let mut sources = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        // 開始タグが見つからない場合は、終了
        let Some(tag_start_offset) = html[index..].find('<') else {
            break;
        };

        let tag_start = index + tag_start_offset;
        let tag_name_start = tag_start + 1;

        // 文字列の一番最後に`<`が置かれているまたは、`<`の次が`/`である場合は、スキップ
        if tag_name_start >= bytes.len() || bytes[tag_name_start] == b'/' {
            index = tag_name_start;
            continue;
        }

        let tag_name_end = read_tag_name_end(html, tag_name_start);

        // タグ名がimgまたはIMGでない場合はスキップ
        if !html[tag_name_start..tag_name_end].eq_ignore_ascii_case("img") {
            index = tag_name_end;
            continue;
        }

        // 終了タグが見つからない場合は、終了
        let Some(tag_end) = find_tag_end(html, tag_name_end) else {
            break;
        };

        let attributes = &html[tag_name_end..tag_end];

        // src属性と値が見つかった場合は、srcの値を返す
        if let Some(source) = read_src_attribute(attributes) {
            sources.push(source);
        }

        index = tag_end + 1;
    }

    sources
}

fn read_tag_name_end(html: &str, start: usize) -> usize {
    html[start..]
        .find(|character: char| {
            character.is_ascii_whitespace() || character == '/' || character == '>'
        })
        .map_or(html.len(), |offset| start + offset)
}

fn find_tag_end(html: &str, start: usize) -> Option<usize> {
    let mut quote = None;

    for (offset, character) in html[start..].char_indices() {
        match (quote, character) {
            // quotationから抜けるとき（保存したquoteと同一）
            (Some(active_quote), current) if active_quote == current => quote = None,
            // quotationに入るとき
            (None, '"' | '\'') => quote = Some(character),
            // タグの終了時
            (None, '>') => return Some(start + offset),
            // それ以外（クォーテーション内に>がある場合など）
            _ => {}
        }
    }

    None
}

fn read_src_attribute(attributes: &str) -> Option<String> {
    let mut index = 0;

    while index < attributes.len() {
        // 属性を読み取る
        index = skip_whitespace(attributes, index);

        if index >= attributes.len() {
            return None;
        }

        let name_start = index;
        let name_end = attributes[index..]
            .find(|character: char| {
                character.is_ascii_whitespace() || character == '=' || character == '/'
            })
            .map_or(attributes.len(), |offset| index + offset);

        if name_start == name_end {
            index += 1;
            continue;
        }

        let name = &attributes[name_start..name_end];

        // = を読み取る
        index = skip_whitespace(attributes, name_end);
        if !attributes[index..].starts_with('=') {
            continue;
        }

        // = に続く属性の値を読み取る
        index = skip_whitespace(attributes, index + 1);

        let (value, next_index) = read_attribute_value(attributes, index);
        index = next_index;

        // 属性がsrcであれば、srcの値を返す
        if name.eq_ignore_ascii_case("src") {
            return Some(value);
        }
    }

    None
}

fn skip_whitespace(text: &str, index: usize) -> usize {
    index
        + text[index..]
        .find(|character: char| !character.is_ascii_whitespace())
        .unwrap_or(text[index..].len())
}

fn read_attribute_value(attributes: &str, index: usize) -> (String, usize) {
    // 属性の値が無い場合は、空文字を返す
    let Some(first) = attributes[index..].chars().next() else {
        return (String::new(), index);
    };

    // クォートがある場合、クォートで囲っている内側の値を返す
    if first == '"' || first == '\'' {
        let value_start = index + first.len_utf8();

        // 終了のクォートが見つからない場合は、最初のクォートより後ろの文字列を全て返す
        let Some(value_end_offset) = attributes[value_start..].find(first) else {
            return (attributes[value_start..].to_owned(), attributes.len());
        };

        let value_end = value_start + value_end_offset;

        return (
            attributes[value_start..value_end].to_owned(),
            value_end + first.len_utf8(),
        );
    }

    // クォートが無い場合、値を返す
    let value_end = attributes[index..]
        .find(|character: char| character.is_ascii_whitespace() || character == '>')
        .map_or(attributes.len(), |offset| index + offset);

    (attributes[index..value_end].to_owned(), value_end)
}

#[cfg(test)]
mod tests {
    use super::{
        collect_image_references, validate_image_references, ImageReference, ImageReferenceKind,
        ImageValidationError,
    };
    use std::path::Path;

    #[test]
    fn detects_markdown_images() {
        let references = collect_image_references("![alt](./images/example.png)\n");

        assert_eq!(
            references,
            vec![ImageReference {
                kind: ImageReferenceKind::Markdown,
                source: "./images/example.png".to_owned(),
            }]
        );
    }

    #[test]
    fn detects_html_img_sources() {
        let references =
            collect_image_references(r#"<img width="500" src="./images/example.webp" alt="alt">"#);

        assert_eq!(
            references,
            vec![ImageReference {
                kind: ImageReferenceKind::Html,
                source: "./images/example.webp".to_owned(),
            }]
        );
    }

    #[test]
    fn ignores_external_urls() {
        let references = collect_image_references(
            "![remote](https://example.com/image.png)\n<img src=\"//example.com/image.png\">\n",
        );

        assert!(references.is_empty());
    }

    #[test]
    fn ignores_image_like_text_inside_code_blocks() {
        let references = collect_image_references(
            "```md\n![alt](./images/example.png)\n<img src=\"./images/example.png\">\n```\n",
        );

        assert!(references.is_empty());
    }

    #[test]
    fn rejects_parent_directory_traversal() {
        let article_path =
            Path::new("tests/fixtures/markdown-valid/articles/markdown-images/article.md");
        let error = validate_image_references(article_path, "![alt](../outside.png)")
            .expect_err("unsafe image path should fail");

        assert!(matches!(error[0], ImageValidationError::UnsafePath { .. }));
    }

    #[test]
    fn rejects_absolute_paths() {
        let article_path =
            Path::new("tests/fixtures/markdown-valid/articles/markdown-images/article.md");
        let error = validate_image_references(article_path, "![alt](/etc/passwd)")
            .expect_err("absolute image path should fail");

        assert!(matches!(
            error[0],
            ImageValidationError::AbsolutePath { .. }
        ));
    }

    #[test]
    fn rejects_windows_style_paths() {
        let article_path =
            Path::new("tests/fixtures/markdown-valid/articles/markdown-images/article.md");
        let error = validate_image_references(article_path, r"![alt](C:\secret.txt)")
            .expect_err("windows style image path should fail");

        assert!(matches!(error[0], ImageValidationError::UnsafePath { .. }));
    }

    #[test]
    fn rejects_unsupported_image_extensions() {
        let article_path =
            Path::new("tests/fixtures/markdown-invalid/articles/unsupported-image/article.md");
        let error = validate_image_references(article_path, "![alt](./images/example.svg)")
            .expect_err("unsupported extension should fail");

        assert!(matches!(
            error[0],
            ImageValidationError::UnsupportedExtension { .. }
        ));
    }

    #[test]
    fn validates_local_images() {
        let article_path =
            Path::new("tests/fixtures/markdown-valid/articles/markdown-images/article.md");
        let markdown =
            "![alt](./images/example.png)\n<img src=\"./images/example.webp\" width=\"500\">\n";

        let references = validate_image_references(article_path, markdown)
            .expect("valid local images should pass");

        assert_eq!(references.len(), 2);
    }
}
