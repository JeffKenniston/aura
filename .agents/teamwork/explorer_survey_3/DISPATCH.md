# Task Assignment: Survey Explorer 3 (AST Blast Radius & MCP Wiring)

- Working Directory: /home/jeff/aura/.agents/teamwork/explorer_survey_3/
- Parent Conversation ID: 966c9504-d054-4c12-b035-b5bb1de49d34
- User Request: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md

Investigate:
1. Locate `tools/ast-blast-radius` and examine its codebase, current tests, and functionality.
2. Investigate how MCP transport and tools are structured in Aura (e.g. MCP request/response handling, JSON schemas, tools registration).
3. Investigate the Blast Radius recursive CTE logic and how it is executed and formatted for MCP.
4. Review existing test harnesses in `tools/ast-blast-radius` and how mock MCP requests should be tested.
5. Relevant skills to inspect:
   - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-net-domain/SKILL.md`
   - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-repo-domain/SKILL.md`
   - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-architecture/SKILL.md`

Produce a comprehensive survey report at `/home/jeff/aura/.agents/teamwork/explorer_survey_3/survey_report.md` and a summary in `handoff.md`.

## 2026-10-09T21:59:27Z
You are Survey Explorer 3 for the M4 Repo Intelligence task in Aura.
Your working directory is: /home/jeff/aura/.agents/teamwork/explorer_survey_3/
You must read ORIGINAL_REQUEST.md at: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md
Your task assignment is in: /home/jeff/aura/.agents/teamwork/explorer_survey_3/DISPATCH.md
Domain skills to inspect:
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-net-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-repo-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-architecture/SKILL.md

Objectives:
1. Locate `tools/ast-blast-radius` and inspect its code, dependencies, crate structure, and current tests.
2. Investigate how MCP transport and tools are structured and registered in Aura (look for MCP schemas, JSON-RPC, request/response formats, and any existing MCP tools).
3. Investigate the Blast Radius recursive CTE logic (SQLite, Cozo, DuckDB, or internal graph query) and how it generates the blast radius report.
4. Investigate what MCP request schema and response format are expected for `ast-blast-radius` and how to route mock MCP requests to the tool in `cargo test`.
5. Document all findings, code locations, type definitions, function signatures, dependencies, and recommended implementation strategy in `/home/jeff/aura/.agents/teamwork/explorer_survey_3/survey_report.md`.
6. Write your handoff summary in `/home/jeff/aura/.agents/teamwork/explorer_survey_3/handoff.md` and message the orchestrator when complete.
