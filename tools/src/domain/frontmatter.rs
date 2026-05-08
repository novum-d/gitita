use serde::Deserialize;
use serde_yaml::Value;
use std::fmt;

#[derive(Debug, Deserialize)]
pub struct Frontmatter {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<Value>>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_as_some_null")]
    pub qiita_id: Option<Value>,

    #[serde(default)]
    pub published: Option<Value>,
}

fn deserialize_null_as_some_null<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Debug, PartialEq, Eq)]
pub enum FrontmatterParseError {
    MissingOpeningDelimiter,
    MissingClosingDelimiter,
    TagsMustBeArray,
    InvalidYaml(String),
}

impl fmt::Display for FrontmatterParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOpeningDelimiter => {
                f.write_str("missing frontmatter opening delimiter `---`")
            }
            Self::MissingClosingDelimiter => {
                f.write_str("missing frontmatter closing delimiter `---`")
            }
            Self::TagsMustBeArray => f.write_str("frontmatter field `tags` must be an array"),
            Self::InvalidYaml(error) => write!(f, "failed to parse frontmatter yaml: {error}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FrontmatterViolation {
    MissingRequiredField(&'static str),
    EmptyRequiredField(&'static str),
    UnsupportedPublishedField,
    InvalidQiitaId,
}

impl fmt::Display for FrontmatterViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequiredField(field) => {
                write!(f, "missing required frontmatter field `{field}`")
            }
            Self::EmptyRequiredField(field) => {
                write!(f, "frontmatter field `{field}` must not be empty")
            }
            Self::UnsupportedPublishedField => {
                f.write_str("unsupported frontmatter field `published`")
            }
            Self::InvalidQiitaId => {
                f.write_str("frontmatter field `qiita_id` must be null or a non-empty value")
            }
        }
    }
}

pub fn parse_frontmatter(content: &str) -> Result<Frontmatter, FrontmatterParseError> {
    let mut lines = content.lines();

    if lines.next() != Some("---") {
        return Err(FrontmatterParseError::MissingOpeningDelimiter);
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
        return Err(FrontmatterParseError::MissingClosingDelimiter);
    }

    let yaml = frontmatter_lines.join("\n");
    serde_yaml::from_str::<Frontmatter>(&yaml).map_err(|error| {
        let error_msg = error.to_string();
        if error_msg.contains("invalid type: string") && error_msg.contains("expected a sequence") {
            FrontmatterParseError::TagsMustBeArray
        } else {
            FrontmatterParseError::InvalidYaml(error_msg)
        }
    })
}

pub fn validate_frontmatter(frontmatter: &Frontmatter) -> Vec<FrontmatterViolation> {
    let mut violations = Vec::new();

    validate_required_string("title", frontmatter.title.as_deref(), &mut violations);
    validate_tags(frontmatter.tags.as_ref(), &mut violations);
    validate_required_string("author", frontmatter.author.as_deref(), &mut violations);
    validate_qiita_id(frontmatter, &mut violations);

    violations
}

fn validate_required_string(
    field: &'static str,
    value: Option<&str>,
    violations: &mut Vec<FrontmatterViolation>,
) {
    match value {
        Some(value) if !value.trim().is_empty() => {}
        Some(_) => violations.push(FrontmatterViolation::EmptyRequiredField(field)),
        None => violations.push(FrontmatterViolation::MissingRequiredField(field)),
    }
}

fn validate_tags(tags: Option<&Vec<Value>>, violations: &mut Vec<FrontmatterViolation>) {
    if tags.is_none() {
        violations.push(FrontmatterViolation::MissingRequiredField("tags"));
    }
}

fn validate_qiita_id(frontmatter: &Frontmatter, violations: &mut Vec<FrontmatterViolation>) {
    if frontmatter.published.is_some() {
        violations.push(FrontmatterViolation::UnsupportedPublishedField);
    }

    match &frontmatter.qiita_id {
        Some(Value::Null) => {}
        Some(Value::String(value)) if !value.trim().is_empty() => {}
        Some(Value::String(_)) => violations.push(FrontmatterViolation::InvalidQiitaId),
        Some(_) => {}
        None => violations.push(FrontmatterViolation::MissingRequiredField("qiita_id")),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_frontmatter, validate_frontmatter, FrontmatterParseError, FrontmatterViolation,
    };

    #[test]
    fn parses_valid_frontmatter() {
        let content = r#"---
title: "Valid Article"
tags:
  - rust
author: "codex"
qiita_id: null
---

# Valid Article
"#;

        let frontmatter = parse_frontmatter(content).expect("frontmatter should parse");

        assert!(validate_frontmatter(&frontmatter).is_empty());
    }

    #[test]
    fn rejects_missing_opening_delimiter() {
        let error = parse_frontmatter("title: missing\n---").expect_err("frontmatter should fail");

        assert_eq!(error, FrontmatterParseError::MissingOpeningDelimiter);
    }

    #[test]
    fn rejects_missing_closing_delimiter() {
        let error = parse_frontmatter("---\ntitle: missing").expect_err("frontmatter should fail");

        assert_eq!(error, FrontmatterParseError::MissingClosingDelimiter);
    }

    #[test]
    fn reports_domain_violations_without_io() {
        let content = r#"---
tags:
  - rust
author: ""
qiita_id: ""
published: true
---
"#;

        let frontmatter = parse_frontmatter(content).expect("frontmatter should parse");

        assert_eq!(
            validate_frontmatter(&frontmatter),
            vec![
                FrontmatterViolation::MissingRequiredField("title"),
                FrontmatterViolation::EmptyRequiredField("author"),
                FrontmatterViolation::UnsupportedPublishedField,
                FrontmatterViolation::InvalidQiitaId,
            ]
        );
    }
}
