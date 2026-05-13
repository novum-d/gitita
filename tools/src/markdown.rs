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
pub struct ResolvedImageReference {
    pub kind: ImageReferenceKind,
    pub source: String,
    pub image_path: PathBuf,
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
    resolve_image_references(article_path, markdown).map(|references| {
        references
            .into_iter()
            .map(|reference| ImageReference {
                kind: reference.kind,
                source: reference.source,
            })
            .collect()
    })
}

pub fn resolve_image_references(
    article_path: &Path,
    markdown: &str,
) -> Result<Vec<ResolvedImageReference>, Vec<ImageValidationError>> {
    let references = collect_image_references(markdown);
    let mut errors = Vec::new();
    let mut resolved_references = Vec::new();

    for reference in references {
        match validate_image_reference(article_path, &reference.source) {
            Ok(image_path) => resolved_references.push(ResolvedImageReference {
                kind: reference.kind,
                source: reference.source,
                image_path,
            }),
            Err(error) => errors.push(error),
        }
    }

    if errors.is_empty() {
        Ok(resolved_references)
    } else {
        Err(errors)
    }
}

pub fn replace_image_sources(markdown: &str, replacements: &[(String, String)]) -> String {
    if replacements.is_empty() {
        return markdown.to_owned();
    }

    // 元の Markdown と同等以上の長さになることが予想されるため、あらかじめメモリ領域を確保する
    let mut output = String::with_capacity(markdown.len());
    let mut in_fenced_code = false;

    for line in markdown.split_inclusive('\n') {  // 改行コード付きでループ
        // 純粋なテキストのみを安全にパースするために、一旦、コンテンツと改行コード(CRLF, LF)を分ける
        let content = line
            .strip_suffix("\r\n")
            .or_else(|| line.strip_suffix('\n'))
            .unwrap_or(line);
        let ending = &line[content.len()..];

        // コードブロック内での画像置換処理を避けるためにコードブロック判定を行い、そのまま出力
        if is_fence_line(content) {
            in_fenced_code = !in_fenced_code;
            output.push_str(line);
            continue;
        }

        if in_fenced_code {
            output.push_str(line);
            continue;
        }

        // コードブロック外での画像置換処理を行い、出力
        output.push_str(&replace_image_sources_in_line(content, replacements));

        // 改行コードを復元
        output.push_str(ending);
    }

    output
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
        index = skip_whitespace(attributes, index);

        if index >= attributes.len() {
            return None;
        }

        // 属性を読み取る
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

fn is_fence_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn replace_image_sources_in_line(line: &str, replacements: &[(String, String)]) -> String {
    let markdown_replaced = replace_markdown_image_sources_in_line(line, replacements);
    replace_html_image_sources_in_line(&markdown_replaced, replacements)
}

fn replacement_for<'a>(source: &str, replacements: &'a [(String, String)]) -> Option<&'a str> {
    replacements
        .iter()
        .find(|(from, _)| from == source)
        .map(|(_, to)| to.as_str())
}

const IMAGE_PREFIX: &str = "![";
const LABEL_SUFFIX: &str = "](";
const IMAGE_SUFFIX: &str = ")";

fn replace_markdown_image_sources_in_line(line: &str, replacements: &[(String, String)]) -> String {
    // あらかじめ1行分のメモリを確保
    let mut output = String::with_capacity(line.len());

    let mut index = 0;
    while let Some(start_offset) = line[index..].find(IMAGE_PREFIX) {
        let start = index + start_offset;

        // 画像マークダウンのプレフィックス`!`より前の文字列を退避
        output.push_str(&line[index..start]);

        // 画像マークダウンのラベルのサフィックス`](`が見つからない場合、画像マークダウンとして扱わない
        let Some(label_end_offset) = line[start + IMAGE_PREFIX.len()..].find(LABEL_SUFFIX) else {
            output.push_str(&line[start..]);
            return output;
        };

        // 画像マークダウンのサフィックス`)`が見つからない場合、画像マークダウンとして扱わない
        let source_start = start + IMAGE_PREFIX.len() + label_end_offset + LABEL_SUFFIX.len();
        let Some(source_end_offset) = line[source_start..].find(IMAGE_SUFFIX) else {
            output.push_str(&line[start..]);
            return output;
        };

        // `![...](`の文字列を退避
        let source_end = source_start + source_end_offset;
        output.push_str(&line[start..source_start]);

        // srcから画像パスとツールチップを取得
        let source = &line[source_start..source_end];
        let (source_path, source_suffix) = markdown_destination_parts(source);

        // ローカルの画像パスとリモートの画像パスを置換
        if let Some(replacement) = replacement_for(source_path, replacements) {
            output.push_str(replacement);
            output.push_str(source_suffix);
        } else {
            output.push_str(source);
        }

        // 画像マークダウン記述のサフィックス`)`で最後に閉じる
        output.push_str(IMAGE_SUFFIX);

        // 次の文字列を読み込むために`)`の後ろにインデックスを置く
        index = source_end + IMAGE_SUFFIX.len();
    }

    output.push_str(&line[index..]);
    output
}

fn markdown_destination_parts(destination: &str) -> (&str, &str) {
    // ツールチップが存在しない場合、画像パスと空のツールチップを返す
    let Some(split_index) = destination.find(char::is_whitespace) else {
        return (destination, "");
    };

    // ツールチップが存在する場合、画像パスとツールチップを分割して返す
    (&destination[..split_index], &destination[split_index..])
}

fn replace_html_image_sources_in_line(line: &str, replacements: &[(String, String)]) -> String {
    // あらかじめ1行分のメモリを確保
    let mut output = String::with_capacity(line.len());

    let mut index = 0;
    let bytes = line.as_bytes();

    while index < bytes.len() {

        // `<`が見つからない場合、imgタグとして扱わない
        let Some(tag_start_offset) = line[index..].find('<') else {
            output.push_str(&line[index..]);
            return output;
        };

        // `<`より前の文字列を退避
        let tag_start = index + tag_start_offset;
        output.push_str(&line[index..tag_start]);

        // タグ開始直後に 途切れているまたは`/`が続く場合は、スキップ
        let tag_name_start = tag_start + '<'.len_utf8();
        if tag_name_start >= bytes.len() || bytes[tag_name_start] == b'/' {
            output.push('<');
            index = tag_name_start;
            continue;
        }

        // imgタグでない場合は、スキップ
        let tag_name_end = read_tag_name_end(line, tag_name_start);
        if !line[tag_name_start..tag_name_end].eq_ignore_ascii_case("img") {
            output.push_str(&line[tag_start..tag_name_end]);
            index = tag_name_end;
            continue;
        }

        // タグ名の終わりが見つからない場合、imgタグとして扱わない
        let Some(tag_end) = find_tag_end(line, tag_name_end) else {
            output.push_str(&line[tag_start..]);
            return output;
        };

        // ローカルの画像パスとリモートの画像パスを置換
        let tag = &line[tag_start..=tag_end];
        if let Some((source_start, source_end, source)) = find_src_attribute_value(tag) {
            output.push_str(&tag[..source_start]);
            if let Some(replacement) = replacement_for(&source, replacements) {
                output.push_str(replacement);
            } else {
                output.push_str(&source);
            }
            output.push_str(&tag[source_end..]);
        } else {
            output.push_str(tag);
        }

        index = tag_end + '>'.len_utf8();
    }

    output
}

fn find_src_attribute_value(attributes_with_tag: &str) -> Option<(usize, usize, String)> {
    let tag_name_end = read_tag_name_end(attributes_with_tag, '<'.len_utf8());
    let mut index = tag_name_end;

    while index < attributes_with_tag.len() {
        index = skip_whitespace(attributes_with_tag, index);

        // src属性を見つけられないまま「最後の文字列」または「>」タグに到達してしまった場合、探索を終了する
        if index >= attributes_with_tag.len() || attributes_with_tag[index..].starts_with('>') {
            return None;
        }

        // 属性名を読み取る
        let name_start = index;
        let name_end = attributes_with_tag[index..]
            .find(|character: char| {
                character.is_ascii_whitespace() || character == '=' || character == '/'
            })
            .map_or(attributes_with_tag.len(), |offset| index + offset);

        // 開始・終了が`=`または`/`（空白はすでにskip_whitespaceによりスキップ）の場合、無効な記号としてスキップ
        if name_start == name_end {
            index += 1; // `=`と`/`は1バイト
            continue;
        }

        // 属性名を取得
        let name = &attributes_with_tag[name_start..name_end];

        // 属性名の次の`=`が無い場合は、スキップ
        index = skip_whitespace(attributes_with_tag, name_end);
        if !attributes_with_tag[index..].starts_with('=') {
            continue;
        }

        // 属性値を取得
        index = skip_whitespace(attributes_with_tag, index + '='.len_utf8());
        let (value, value_start, value_end, next_index) =
            read_attribute_value_with_range(attributes_with_tag, index);
        index = next_index;

        // 属性名が`src`の場合、属性値を返す
        if name.eq_ignore_ascii_case("src") {
            return Some((value_start, value_end, value));
        }
    }

    None
}

fn read_attribute_value_with_range(text: &str, index: usize) -> (String, usize, usize, usize) {
    // `<img src=`のような不完全なHTMLタグを読み込んでしまった場合は、空の属性値で返す
    let Some(first) = text[index..].chars().next() else {
        return (String::new(), index, index, index);
    };

    // 引用符あり
    if first == '"' || first == '\'' {
        let value_start = index + first.len_utf8();

        // 閉じる引用符が見つからない場合、不完全なタグとして扱う
        let Some(value_end_offset) = text[value_start..].find(first) else {
            return (
                text[value_start..].to_owned(),
                value_start,
                text.len(),
                text.len(),
            );
        };

        // 閉じる引用符が見つかった場合、その位置まで属性値として返す
        let value_end = value_start + value_end_offset;
        return (
            text[value_start..value_end].to_owned(),
            value_start,
            value_end,
            value_end + first.len_utf8(),
        );
    }

    // 引用符なし
    let value_end = text[index..]
        .find(|character: char| character.is_ascii_whitespace() || character == '>')
        .map_or(text.len(), |offset| index + offset);

    (
        text[index..value_end].to_owned(),   // 属性値
        index,                               // 属性値の開始位置
        value_end,                           // 属性値の終了位置
        value_end,                           // 属性値のの次の位置
    )
}

#[cfg(test)]
mod tests {
    use super::{
        collect_image_references, replace_image_sources, resolve_image_references,
        validate_image_references, ImageReference, ImageReferenceKind, ImageValidationError,
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

    #[test]
    fn resolves_local_images_to_files() {
        let article_path =
            Path::new("tests/fixtures/markdown-valid/articles/markdown-images/article.md");
        let markdown =
            "![alt](./images/example.png)\n<img src=\"./images/example.webp\" width=\"500\">\n";

        let references =
            resolve_image_references(article_path, markdown).expect("images should resolve");

        assert_eq!(references.len(), 2);
        assert!(references[0].image_path.ends_with("images/example.png"));
        assert!(references[1].image_path.ends_with("images/example.webp"));
    }

    #[test]
    fn replaces_only_targeted_image_sources_in_memory() {
        let markdown = concat!(
            "![local](./images/example.png)\n",
            "<img src=\"./images/example.webp\" width=\"500\">\n",
            "![remote](https://example.com/image.png)\n",
            "```md\n",
            "![code](./images/example.png)\n",
            "<img src=\"./images/example.webp\">\n",
            "```\n"
        );

        let replaced = replace_image_sources(
            markdown,
            &[
                (
                    "./images/example.png".to_owned(),
                    "https://img.qiita.com/uploaded.png".to_owned(),
                ),
                (
                    "./images/example.webp".to_owned(),
                    "https://img.qiita.com/uploaded.webp".to_owned(),
                ),
            ],
        );

        assert!(replaced.contains("![local](https://img.qiita.com/uploaded.png)"));
        assert!(replaced.contains(r#"<img src="https://img.qiita.com/uploaded.webp" width="500">"#));
        assert!(replaced.contains("![remote](https://example.com/image.png)"));
        assert!(replaced.contains("![code](./images/example.png)"));
        assert!(replaced.contains(r#"<img src="./images/example.webp">"#));
    }

    #[test]
    fn preserves_markdown_image_title_when_replacing_source() {
        let replaced = replace_image_sources(
            r#"![local](./images/example.png "title")"#,
            &[(
                "./images/example.png".to_owned(),
                "https://img.qiita.com/uploaded.png".to_owned(),
            )],
        );

        assert_eq!(
            replaced,
            r#"![local](https://img.qiita.com/uploaded.png "title")"#
        );
    }
}
