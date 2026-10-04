---
name: ast-blast-radius
description: Analyzes repository syntax trees using recursive Common Table Expressions (CTEs) against SQLite to quantify the dependency blast radius of proposed refactorings.
version: 1.0
---

# AST Blast-Radius Analysis Skill

## Overview
This skill executes structural static analysis against the codebase knowledge graph prior to modifying existing code structures. It uses `tree-sitter` CST metadata indexed in SQLite to compute all reverse callers and dependent modules.

## Workflow Instructions
1. **Identify Target Symbols**:
   - Extract the specific structs, functions, traits, or classes targeted for refactoring.

2. **Execute Recursive CTE Query**:
   - Query the local SQLite knowledge graph (`.agents/knowledge_graph.sqlite`) using recursive CTEs to trace upstream callers and cross-file import references.
   - Example query logic:
     ```sql
     WITH RECURSIVE CallChain AS (
       SELECT caller_id, callee_id, 1 AS depth
       FROM symbol_references
       WHERE callee_id = :target_symbol_id
       UNION ALL
       SELECT sr.caller_id, sr.callee_id, cc.depth + 1
       FROM symbol_references sr
       JOIN CallChain cc ON sr.callee_id = cc.caller_id
       WHERE cc.depth < 10
     )
     SELECT DISTINCT caller_id, depth FROM CallChain;
     ```

3. **Threshold Assessment**:
   - **Low Blast Radius (<= 3 callers, 1 module)**: Proceed with standard automated edit and localized testing.
   - **Medium Blast Radius (4-10 callers, 2-3 modules)**: Require explicit unit test updates across all callers before editing.
   - **High Blast Radius (> 10 callers or cross-boundary)**: Halt execution. Escalate cognitive tier to Gemini 3.1 Pro and present risk breakdown to the user.
