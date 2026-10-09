# AGENTS.md — Foundational Repository Constitution for Aura

<guidelines>
- **Zero-Trust Identity**: All execution contexts require a SPIFFE SVID. Do not attempt to bypass auth mechanisms.
- **Verification Gates**: Code must be production-ready. ABSOLUTELY NO mocks, stubs, or TODOs.
- **No Manual Formatting**: Do not manually format code. Formatting is handled deterministically via `PostToolUse` lifecycle hooks (`cargo`, `ruff`).
- **AST Blast-Radius**: Structural refactors must undergo AST Blast-Radius CTE analysis before applying.
- **Circuit Breaker**: If a task fails or errors continuously, do not blindly retry permutations. Halt and ask the user for clarification.
</guidelines>
