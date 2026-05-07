# DECISIONS.md

## Purpose

Record important decisions.

---

## Source of Truth

Git is the source of truth.

Qiita is a deployment target.

---

## qiita_id

Stored in frontmatter.

Used to:

* Prevent duplicate posts
* Enable updates

---

## CI Policy

Keep CI simple:

* check
* publish

The review workflow should run validation and Rust checks on pull requests.

---

## Image Handling

MVP:

* Use Qiita upload API from Rust
* Replace local image paths in memory during publish

Future:

* Consider upload deduplication if needed
* Consider `.gititaignore` support if article assets need repository-local ignore rules
