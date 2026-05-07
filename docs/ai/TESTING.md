# TESTING.md

## Required Commands

Run before PR:

```bash
cargo check
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
````

---

## Scope

Test:

* CLI behavior
* Frontmatter parsing

---

## Rules

* Do not disable tests
* Add tests for new logic

---

## Manual Check

* Run CLI locally
* Confirm publish flow
