# AGENTS.md

## Purpose

This repository uses Codex to support:

1. Article creation
2. Small improvements
3. Validation fixes
4. Draft PR creation

Codex must prioritize safety, small changes, and minimal impact.

---

## Required AI Documentation

Before making changes, Codex must read:

* docs/ai/ARCHITECTURE.md
* docs/ai/DECISIONS.md
* docs/ai/TESTING.md

---

## Allowed Changes

Codex may modify:

* articles/**
* tools/**
* tests/**
* docs/**
* Cargo.toml
* Cargo.lock
* README.md

Codex should prefer small changes and avoid touching unrelated files.

---

## Forbidden Changes

Codex must not modify:

* .github/workflows/**
* secrets/**
* .env*
* production configs

---

## Human Approval Required

The following require approval:

* Workflow changes
* Changes that deviate from the defined Qiita API integration policy

New Rust dependencies are allowed when they support the documented architecture
and remain scoped to the requested implementation.

---

## Allowed Scope

Prefer:

* Small fixes
* Article improvements
* Validation fixes
* Documentation updates

Avoid:

* Large refactors
* Multi-feature PRs

---

## Article Rules

Articles are stored in:

```text
articles/<slug>/article.md
```

Optional article images are stored in:

```text
articles/<slug>/images/
```

Each article must include frontmatter:

```yaml
title: ""
tags: []
author: ""
qiita_id: null
```

Rules:

* One PR should modify only one article
* Do not remove existing frontmatter fields
* Do not change qiita_id manually

---

## Required Checks

Before PR:

```bash
cargo check
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

---

## Pull Request Rules

All PRs must:

* Be Draft PRs
* Be written in Japanese
* Include summary
* Include changes
* Include manual verification steps
* PR title must exactly match the related Issue title
* Follow AGENTS.md naming conventions

---

## Safety Rules

Codex must stop if:

* Scope unclear
* Approval required

---

## Preferred Strategy

1. Read docs
2. Make minimal change
3. Validate
4. Create Draft PR

---

## Immutable Governance Files

Codex must NOT modify the following files unless explicitly requested:

```text
.codex/prompt.md
AGENTS.md
.github/CODEOWNERS
.github/pull_request_template.md
.github/ISSUE_TEMPLATE/**
.github/workflows/**
```

These files define repository governance, architecture, and workflow policies.

---

## Naming Conventions

Use clear and human-readable names for:

* branches
* issues
* pull requests

Avoid vague names such as:

* fix
* update
* misc
* changes

Prefer descriptive names such as:

* add-frontmatter-parser
* implement-image-upload
* add-publish-workflow

---

## Branch Naming

Use:

```text
<type>/<short-description>
```

Examples:

```text
feat/add-frontmatter-parser
feat/implement-image-upload
fix/publish-error-handling
chore/update-workflows
```

Keep names concise and readable.

---

## Issue and Pull Request Naming

Issue titles and PR titles MUST be identical.

Examples:

```text
[impl] add frontmatter parser
[impl] implement image upload
[fix] prevent publish workflow loop
```

The PR title must exactly match the related issue title.

Avoid unrelated wording changes.

---

## Pull Request Template

When creating pull requests, always use:

```text
.github/pull_request_template.md
```

All pull requests must follow the repository PR template.

---
