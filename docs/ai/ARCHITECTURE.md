# ARCHITECTURE.md

## Purpose

This project manages Qiita articles using Git + CI.

---

## Structure

```
.
├── articles/        # Markdown articles by slug
│   └── <slug>/
│       ├── article.md
│       └── images/
├── tools/           # Rust CLI
└── docs/
```

---

## Responsibilities

### articles/

* Source of truth for articles
* Stores each article as `articles/<slug>/article.md`
* Contains frontmatter with `title`, `tags`, `author`, and `qiita_id`

### tools/

* CLI for validation and publishing
* Handles qiita_id logic

---

## Flow

1. Create PR
2. Run checks
3. Merge to main
4. CI publishes article

---

## Rules

* Do not introduce new top-level directories
* Keep logic inside tools/
* Articles must remain simple Markdown
