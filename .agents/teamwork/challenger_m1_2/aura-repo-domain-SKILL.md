---
name: aura-repo-domain
description: Repository Intelligence constraints. Activate this skill when working
  on the repo domain.
---
# REPO Domain Rules
- **Parsing**: tree-sitter into incremental CSTs. Keep original whitespace/comments.
- **Indexing**: Parallel with rayon. Hash with BLAKE3.
- **Storage**: SQLite (BM25 + vector).
- **Conflict**: Recalculate BLAKE3 tree before patch apply to detect concurrent commits.
- **CI**: Agent CST edits must pass native linting hooks in sandbox before committing.
