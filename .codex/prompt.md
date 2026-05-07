# gitita Codex Prompt

## Overview

gitita is a Git-based Qiita article publishing system.

This repository manages Qiita articles using:

- GitHub Pull Requests
- GitHub Actions
- Rust CLI
- Qiita APIs

Git is the source of truth.

Qiita is the deployment target.

The system should remain lightweight and simple.

Avoid over-engineering.

---

# Architecture

## Article Structure

```text
articles/<slug>/
├── article.md
└── images/
```

Example:

```text
articles/rust-clap-intro/
├── article.md
└── images/
    ├── image1.png
    └── image2.png
```

Directory name is treated as slug.

---

# Article File Naming

Each article directory must contain:

```text
article.md
```

Do not introduce alternative article filenames.

---

# Frontmatter

Each article must contain:

```yaml
---
title: ""
tags: []
author: ""
qiita_id: null
---
```

Rules:

- `title` is required
- `tags` is required
- `author` is required
- `qiita_id` is managed automatically
- Never edit `qiita_id` manually
- Existing article author must not change

---

# Frontmatter Formatting

Prefer stable frontmatter ordering:

```yaml
title:
tags:
author:
qiita_id:
```

---

# Publish Flow

```text
PR
↓
Review CI
↓
main merge
↓
gitita publish
↓
Qiita publish/update
```

---

# Publish Rules

## New Articles

A new article means:

```yaml
qiita_id: null
```

Rules:

- One PR may contain only one new article
- Multiple new articles must fail CI

---

## Existing Articles

An existing article means:

```yaml
qiita_id != null
```

Rules:

- Multiple article updates are allowed
- Updating existing articles automatically updates Qiita

---

# Ownership Rules

The PR author must match:

```yaml
author:
```

If not matched:

- CI must fail

Existing article author changes are forbidden.

---

# Draft Policy

Use GitHub Draft PR.

Do not introduce custom draft fields.

---

# Diff Detection

Only changed articles should be published.

Use git diff for change detection.

Ignore unrelated file changes.

Only process:

```text
articles/<slug>/
```

---

# Rename Rules

Article directory rename is forbidden.

Example:

```text
articles/rust/
→
articles/rust-intro/
```

must fail CI.

Reason:

- directory name is treated as slug
- URL stability must be preserved

---

# Image Rules

## Supported Formats

Only formats supported by Qiita are allowed.

Examples:

- png
- jpg
- jpeg
- gif
- webp
- tiff
- avif

Unsupported formats must fail CI.

Reference:
<https://help.qiita.com/ja/articles/qiita-image-upload>

---

## Supported Syntax

### Markdown image

```md
![alt](./images/example.png)
```

### HTML image

```html
<img src="./images/example.png" width="500">
```

Rules:

- HTML img tags are allowed
- width attributes must be preserved
- only local relative paths are upload targets
- external URLs should be ignored

---

# Path Safety

Image paths must remain inside:

```text
articles/<slug>/
```

Reject:

- parent directory traversal
- absolute paths

Examples:

```text
../../foo.png
/etc/passwd
C:\secret.txt
```

must fail validation.

---

# Symlink Policy

Do not follow symbolic links.

Symlinked files should fail validation.

---

# File Size Policy

Warn for unusually large image files.

Do not fail CI only because of size in MVP.

---

# Image Upload Strategy

Images are uploaded using Qiita upload APIs.

Rules:

- Upload images on every publish
- Do not cache uploaded images
- Upload failure must fail article publish

Important:

Git-managed markdown files must NOT be rewritten.

Local image paths must remain unchanged in Git.

Image URLs should only be replaced in-memory during publish processing.

---

# Markdown Parsing

Use:

```text
pulldown-cmark
```

Purpose:

- detect markdown images safely
- avoid regex-only parsing
- ignore code blocks safely

HTML img tags may be parsed separately.

Do not implement markdown parsing using regex only.

---

# Markdown Processing Rules

Markdown processing should:

- parse markdown safely
- detect local images
- preserve HTML img width attributes
- avoid modifying original files

---

# Markdown Compatibility

Preserve original markdown formatting as much as possible.

Avoid unnecessary markdown rewriting.

---

# API Policy

Use Qiita APIs directly from Rust.

Do NOT depend on Qiita CLI.

Use:

- Item API
- Upload API

Reference:
<https://qiita.com/api/v2/docs>

---

# CLI Commands

Supported commands:

```bash
gitita check
gitita publish
gitita publish --dry-run
```

Rules:

- keep CLI surface minimal
- avoid unnecessary subcommands
- avoid interactive prompts

---

# Exit Code Policy

Return non-zero exit codes for:

- validation failures
- publish failures
- API failures

Return zero for:

- successful checks
- warning-only situations

---

# Configuration

Use environment variables.

Examples:

```text
QIITA_TOKEN
GITHUB_TOKEN
```

Do not introduce custom config files in MVP.

---

# Project Structure

Recommended structure:

```text
tools/src/
├── article/
├── cli/
├── diff/
├── github/
├── markdown/
├── qiita/
├── workflow/
└── main.rs
```

Responsibilities:

- article/: frontmatter and article models
- cli/: command handling
- diff/: changed article detection
- github/: GitHub integrations
- markdown/: markdown and image parsing
- qiita/: Qiita API client
- workflow/: publish orchestration

Keep responsibilities isolated.

---

# Qiita API Client Design

The Qiita client should:

- upload images
- create articles
- update articles

Avoid mixing:

- markdown parsing
- git logic
- API logic

Keep API logic isolated.

---

# Async Policy

Use tokio async runtime.

Network operations should be async.

Avoid unnecessary async abstractions.

---

# State Management

Do not introduce:

- databases
- persistent caches
- background workers

The repository itself is the source of truth.

---

# Error Handling

Use structured error handling.

Prefer:

- anyhow for application errors
- thiserror for domain errors

Avoid:

- unwrap()
- expect() in production logic

---

# Panic Policy

Production paths should return errors instead of panicking.

---

# Logging

Use structured logging.

Recommended:

```text
tracing
```

Examples:

```text
INFO uploading image path=...
INFO publishing article slug=...
ERROR upload failed status=...
```

Logs should help debugging CI failures.

---

# Secret Handling

Never log:

- API tokens
- Authorization headers
- secret environment variables

Mask sensitive values in logs.

---

# Timeout Policy

Network requests should use explicit timeout settings.

Recommended default:

```text
30 seconds
```

---

# Retry Policy

Do not implement automatic retries in MVP.

GitHub Actions rerun is sufficient.

---

# Rate Limit Handling

If Qiita API rate limits occur:

- fail gracefully
- provide readable error messages

---

# Deterministic Behavior

Given the same repository state,
gitita should produce the same outputs.

Avoid non-deterministic behavior.

---

# Time Usage

Avoid embedding current timestamps into repository files.

---

# Temporary Files

Temporary files must:

- remain outside repository directories
- be cleaned automatically

---

# Performance Philosophy

Prefer simple and memory-efficient processing.

Avoid unnecessary full-repository loading.

---

# Dependency Policy

New Rust dependencies are allowed when they support the documented architecture.

Avoid introducing heavy frameworks.

Prefer focused dependencies that directly support the requested implementation.

---

# Future: .gititaignore

Optional ignore rules similar to `.gitignore` may be added after the MVP.

Example:

```text
*.psd
*.ai
.DS_Store
```

If implemented, ignored files must not be uploaded or validated.

---

# Dry Run

Support dry-run mode.

Example:

```bash
gitita publish --dry-run
```

Dry-run should display:

- publish targets
- upload targets
- generated URLs
- article update targets

Dry-run must NOT call publish APIs.

---

# Automated Commit Rules

GitHub Actions may update:

- qiita_id

GitHub Actions must NOT modify:

- article body
- image references
- frontmatter except qiita_id

---

# Bot Loop Prevention

Prevent infinite workflow loops caused by bot commits.

Example:

```yaml
if: github.actor != 'github-actions[bot]'
```

Bot-generated commits must not trigger publish loops.

---

# Concurrency

Publish workflows must not run concurrently.

Use GitHub Actions concurrency control.

Example:

```yaml
concurrency:
  group: gitita-publish
  cancel-in-progress: false
```

---

# CI Structure

## review.yml

Triggered on:

- pull_request

Responsibilities:

- validate frontmatter
- validate ownership
- validate image formats
- validate article count
- detect forbidden rename
- run cargo check
- run cargo fmt --check
- run cargo clippy --all-targets --all-features -- -D warnings
- run cargo test

This workflow must NOT publish articles.

---

## publish.yml

Triggered on:

- push to main

Responsibilities:

- detect changed articles
- upload images
- publish/update Qiita articles
- update qiita_id
- create failure issues

Only this workflow may publish articles.

---

# Security Rules

Publish workflows must run only on:

- main branch
- trusted repository context

Fork pull requests must never access:

- Qiita API tokens
- publish workflows

---

# qiita_id Update Rules

After successful new article publish:

- update qiita_id automatically
- commit changes using GitHub Actions bot
- push directly to main

Do not modify article body during this process.

---

# Publish Failure Handling

If publish fails:

- CI must fail
- GitHub Issue must be created

Issue title format:

```text
[publish-failed] <slug>
```

Rules:

- avoid duplicate issues
- reuse existing issue if possible
- add comments to existing issue if already created

---

# Assignee Rules

Publish failure issues should assign:

```yaml
author:
```

to GitHub assignee.

If assignee assignment fails:

- continue issue creation
- do not fail the workflow

---

# Validation Messages

Validation errors should:

- clearly explain the problem
- include target article path
- include actionable fixes

---

# Encoding

All markdown files must use UTF-8 encoding.

---

# Line Endings

Prefer LF line endings.

---

# CI Severity Policy

Fail CI only for:

- invalid frontmatter
- unsupported image formats
- ownership violations
- forbidden rename
- multiple new articles
- publish failures

Prefer warnings for:

- large articles
- unusual tags
- style recommendations

Avoid overly strict lint rules.

---

# Testing

Run before PR:

```bash
cargo check
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

---

# Test Fixtures

Use fixture-based tests.

Recommended structure:

```text
tests/fixtures/
```

Examples:

```text
valid_article/
invalid_frontmatter/
unsupported_image/
rename_case/
```

---

# Recommended Rust Crates

Recommended crates:

```toml
reqwest
serde
serde_json
serde_yaml
tokio
pulldown-cmark
anyhow
thiserror
tracing
```

---

# Coding Guidelines

Prefer:

- small functions
- explicit logic
- readable code
- simple modules

Avoid:

- unnecessary generics
- macro-heavy designs
- large abstractions
- premature optimization

---

# Documentation

Document public modules and important workflows.

Keep documentation concise and practical.

---

# Pull Request Rules

All PRs should:

- remain small
- focus on a single purpose
- avoid unrelated changes

Prefer Draft PRs.

PR descriptions should include:

- summary
- changes
- manual verification steps

---

# Versioning

Use semantic versioning.

Avoid breaking CLI behavior in minor releases.

---

# Backward Compatibility

Prefer additive changes.

Avoid changing:

- article directory structure
- frontmatter keys
- CLI command behavior

without explicit approval.

---

# Safety Rules

Codex must stop and ask for approval if:

- adding databases
- adding caching systems
- introducing background workers
- changing workflows
- changing ownership rules
- changing publish behavior
- adding new infrastructure

---

# Immutable Files

Codex must NOT modify the following files unless explicitly requested:

```text
.codex/prompt.md
.github/pull_request_template.md
.github/ISSUE_TEMPLATE/*
.github/CODEOWNERS
```

These files define repository governance and architecture rules.

---

# MVP Priorities

Prioritize implementation order:

1. frontmatter parser
2. review workflow
3. diff detector
4. markdown parser
5. image upload
6. publish API
7. author validation
8. failure issue automation
9. publish workflow

Avoid over-engineering.

Keep the implementation simple and maintainable.
