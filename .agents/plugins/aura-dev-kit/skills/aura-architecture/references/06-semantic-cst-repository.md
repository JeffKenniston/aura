---
title: Semantic Repository Intelligence and CST Knowledge Graphing
description: Regulates tree-sitter Concrete Syntax Tree parsing, Merkle change tracking, and SQLite knowledge graph indexing.
tags: [cst, tree-sitter, blake3, merkle-tree, sqlite, blast-radius]
version: 1.0
---

# Semantic Repository Intelligence & CST Knowledge Graphing

## 1. Concrete Syntax Trees (CST) vs. Lossy AST
- Source code parsing must use `tree-sitter` in the Rust host to construct Concrete Syntax Trees.
- CSTs must preserve all whitespace, indentation, comments, and punctuation.
- When generating patches, agents rewrite specific syntax nodes without altering untouched code formatting or developer styling.

## 2. Incremental Indexing via Merkle Trees
- Repositories are parsed concurrently using `rayon`.
- Files are hashed into a BLAKE3 Merkle tree to detect localized delta changes in milliseconds without triggering full repository rebuilds.

## 3. SQLite Knowledge Graph & AST Blast Radius
- Parsed code entities, call graphs, and symbol definitions are indexed into an SQLite database with hybrid BM25 full-text and vector embeddings.
- A three-pass resolution pass resolves bare symbols and cross-file imports.
- **AST-Blast-Radius Pre-Check**:
  - Before applying any structural refactoring or API signature modification, agents must execute recursive Common Table Expression (CTE) queries against the SQLite graph.
  - If the blast radius exceeds configured threshold limits (e.g. >10 downstream modules impacted), the refactoring must be halted, submitted for human review, and escalated to Gemini 3.1 Pro.
