---
name: Implementation
about: Implementation task for gitita
title: "[impl] "
labels: ["implementation"]
assignees: []
---

# Summary

Describe the implementation goal clearly.

---

# Scope

Describe the exact implementation scope.

Out of scope items should NOT be implemented.

---

# Requirements

List concrete requirements.

- [ ]
- [ ]
- [ ]

---

# Acceptance Criteria

Implementation is complete when:

- [ ]
- [ ]
- [ ]

---

# Technical Notes

Relevant architecture and implementation notes.

Examples:

- use existing module structure
- avoid introducing new abstractions
- preserve current workflow behavior

---

# Constraints

The implementation MUST follow `.codex/prompt.md`.

Do NOT:

- modify architecture decisions
- introduce databases
- introduce caching systems
- introduce background workers
- modify publish workflows unless explicitly requested
- modify ownership rules
- modify `.codex/prompt.md`
- add unrelated changes
 
Do NOT modify governance files:

- .codex/prompt.md
- .github/pull_request_template.md
- .github/ISSUE_TEMPLATE/*
- .github/CODEOWNERS



Keep implementation minimal and focused.

---

# Testing

Required validations before completion:

```bash
cargo check
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

---

# Deliverables

Expected outputs:

- implementation
- tests
- documentation updates if necessary

---

# Notes for Codex

Prefer:

- small readable functions
- explicit logic
- minimal dependencies

Avoid:

- unnecessary abstractions
- macro-heavy implementations
- speculative features
