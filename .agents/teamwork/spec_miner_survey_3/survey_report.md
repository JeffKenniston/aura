# Phase M3 Specification Survey Report: Cognition, Model Routing, and Token Economics

**Surveyor:** `spec_miner_survey_3` (teamwork_preview_spec_miner)  
**Parent Orchestrator:** `orchestrator_2` (`d71b58bc-d6bb-4674-ac7d-b24b5ce1f6c3`)  
**Scope:** Phase M3 (Cognition: Cognitive Processing & Observability, Model Router, Token Economics, Interactions API, Context Saturation Mitigation)  
**Date:** 2026-10-09  

---

## Executive Summary

This report establishes the authoritative specification survey and code audit for Phase M3 of the Aura Project. The investigation probed:
1. `ORIGINAL_REQUEST.md` (Authoritative user mandate)
2. `ROADMAP.md` (Phase 3 milestone deliverables)
3. `KNOWLEDGE.md` and `RESEARCH.md` (Architectural blueprints and gap analyses)
4. `ADR-002-Identity-Routing-Repo.md` (Identity, routing heuristics, git conflict resolution, and context saturation mitigations)
5. Domain skills: `aura-cog-domain`, `aura-rte-domain`, `aura-architecture/references/05-gemini-interactions-routing.md`, and `07-guardrails-token-economics.md`
6. Microkernel Rust implementations: `aura-core/src/transport/rate_limiter.rs`, `aura-core/src/transport/webhook.rs`, `aura-core/src/config/mod.rs`, and `aura-core/src/knowledge/cache.rs`
7. Python SDK implementations: `aura-sdk/aura_sdk/cognition/` (`gemini.py`, `context.py`, `cache.py`), `aura-sdk/aura_sdk/routing/` (`router.py`), and test suites in `aura-sdk/tests/`.

All 14 Cognition requirements (`COG-001` through `COG-014`), 9 Model Routing requirements (`RTE-001` through `RTE-009`), and 8 Architectural Gap requirements (`GAP-001` through `GAP-008`, with deep emphasis on context saturation and cache mitigation) have been mined, cataloged, cross-referenced with exact code citations (file paths and line numbers), and evaluated for implementation completeness.

---

## Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | Cognition | Gemini Interactions API Client | Stateful agentic interaction turns using `client.aio.interactions.create` without stateless prompt resending | Prompt string or dict, model tier, tools, `file_search_stores`, `background` flag | Async stream of `CognitiveEvent` objects | Raises `RuntimeError` if client unconfigured; retries transient errors (429/5xx) | `ROADMAP.md:26`, `aura-cog-domain/SKILL.md:7`, `gemini.py:281-356` |
| 2 | Cognition | Server-Side State Management | Passes `previous_interaction_id` to continue sessions on Gemini server, leveraging implicit caching and reducing TTFT | Initial prompt, followed by delta turns and `previous_interaction_id` | `interaction_id` echoed across turns; server retrieves history | If session invalid, throws API error | `ROADMAP.md:26`, `RESEARCH.md:57-58`, `gemini.py:199,330-333` |
| 3 | Cognition | SSE Stream Ingestion | Consumes Server-Sent Events, isolating `ModelThought`, `ToolCallRequest`, `TextDelta`, and usage metrics | SSE event stream from Gemini API | `CognitiveEvent` with `EventType.THOUGHT`, `TOOL_CALL`, `TEXT_DELTA`, `INTERACTION_COMPLETED` | Yields `EventType.ERROR` on malformed frames | `ROADMAP.md:27`, `RESEARCH.md:59-60`, `gemini.py:30-58,361-456` |
| 4 | Cognition | Function Call Interception | Microkernel intercepts `step.start` tool calls, validates JSON schema, executes tool in WASM/microVM, and reinjects `function_result` | Tool invocation name & arguments from model | Injected tool result into ongoing session via `previous_interaction_id` | `ToolArgumentValidationError` on schema violation; halts or returns error result | `RESEARCH.md:59-60`, `gemini.py:404-407,458-506` |
| 5 | Cognition | Long-Horizon Background Tasks | Dispatches autonomous workflows with `background=true`, returning interaction ID and tracking completion via Zenoh or polling | Heavy prompt with `background=true` | `interaction.created` with `interaction_id` | Timeout error after configured threshold if neither Zenoh nor polling completes | `ROADMAP.md:30`, `RESEARCH.md:61-64`, `gemini.py:334-356,508-537` |
| 6 | Cognition | Native RAG via File Search Stores | Attaches `file_search_stores` to interaction turns, enabling unstructured context retrieval without separate vector DB | List of store names in `file_search_stores` | `tools` list containing `file_search` tool configuration | Provider error if store not found | `RESEARCH.md:68-70`, `aura-cog-domain/SKILL.md:12`, `gemini.py:313-317` |
| 7 | Cognition | Turn Economics & Metrics | Tracks TTFT, input tokens, output tokens, cached tokens, total tokens, and USD cost per model tier | Model name and token usage counts | `InteractionMetrics` instance stored in history and emitted in completion event | Defaults to standard Flash pricing if model unknown | `RESEARCH.md:57`, `07-guardrails-token-economics.md`, `gemini.py:61-98,414-436` |
| 8 | Cognition | Jittered Exponential Backoff | Resilient retry loop for transient 429 (ResourceExhausted) and 5xx (Unavailable) API errors | API call callable, max retries, base delay | Successful API response | Re-raises exception after max retries exceeded | `gemini.py:259-280`, `test_cognition_gemini.py:224-253` |
| 9 | Cognition | Credential Leak Sanitization | Pre-flight regex scanner detecting Google API keys, Bearer tokens, and generic secrets before dispatch | Outbound prompt string | Sanitized prompt string | Throws `SecurityCredentialLeakError` immediately; blocks request | `STRIDE.md:31`, `aura-cog-domain/SKILL.md:13`, `gemini.py:101-116` |
| 10 | Cognition | Tool Argument Schema Validation | Validates model-provided tool arguments against expected JSON Schema `required` fields and property types | Schema definition dict and parameter dict | None (valid) | Throws `ToolArgumentValidationError` with parameter name and expected type | `gemini.py:119-146,404-407` |
| 11 | Cognition | File Search Store Cleanup | Issues explicit HTTP DELETE calls to delete Gemini API file search stores upon task completion or TTL | Store identifier string | Boolean success; audit log record | Logs warning if remote store deletion fails; does not crash agent | `aura-cog-domain/SKILL.md:12`, `gemini.py:233-257` |
| 12 | Cognition | Tenant Billing Tag Injection | Injects `billing_tenant_id` and `x-tenant-id` into `client_metadata` on every outbound request | Tenant identifier string | Enriched API request kwargs | Throws error if tenant context missing in strict multi-tenant mode | `gemini.py:319-325`, `test_cognition_gemini.py:288-306` |
| 13 | Cognition | Host Semantic Vector Cache | SQLite database with `sqlite-vec` extension and BLAKE3 hashing on Rust host, queried over Zenoh by Python SDK | Prompt string, tenant ID, optional embedding | `CacheLookupResult` (<50ms, 0 tokens) | Gracefully falls back to normal LLM routing on cache miss or Zenoh timeout | `KNOWLEDGE.md:70`, `aura-core/src/knowledge/cache.rs:79-510`, `cache.py:64-328` |
| 14 | Cognition | Shadow Storage Cost Tracking | Host `ShadowTracker` and SDK calculate cumulative Gemini file storage costs from uploaded bytes, TTLs, and pricing | Uploaded file size (bytes), TTL (days), rate per GB-month | Cumulative USD storage cost | Recalculates dynamically on file registration or deletion | `ROADMAP.md:29`, `aura-core/src/transport/rate_limiter.rs:87-176`, `gemini.py:212-231` |
| 15 | Model Router | Four Model Tiers Matrix | Categorizes foundation models into Pro, Flash (default), Flash-Lite, and Deep Research | Target tier enum | Model identifier string (`gemini-3.1-pro`, `gemini-3.8-flash`, etc.) | Throws error if invalid tier requested | `ROADMAP.md:28`, `RESEARCH.md:73-80`, `router.py:20-35` |
| 16 | Model Router | Default Flash Orchestration | Defaults all standard engineering, tool use, and routine tasks to `gemini-3.8-flash` | Prompt string without escalation flags | `{"model": "gemini-3.8-flash"}` | Fallback to Flash-Lite if cost/latency budget exceeded | `RESEARCH.md:77`, `aura-rte-domain/SKILL.md:7`, `router.py:31,302-304` |
| 17 | Model Router | Heuristic Decision Matrix | Rule-based classifier evaluating prompt length, planning keywords, latency budget (<1s), and cost budget (<$0.001) | Prompt, latency budget (ms), cost budget ($) | Selected tier dict (`{"model": ...}`) | Deterministic fallback to Flash tier | `ADR-002:12-13`, `router.py:332-371` |
| 18 | Model Router | Deep Research Explicit Gating | Enforces that Deep Research shall ONLY be triggered via explicit `agent_options={"deep_research": True}` | Prompt, `agent_options` | `{"agent": "deep-research-preview"}` | Prompt text keywords alone are prevented from activating Deep Research | `RESEARCH.md:79`, `aura-rte-domain/SKILL.md:7`, `router.py:305-325` |
| 19 | Model Router | AST Blast Radius Escalation | Evaluates reverse-dependencies from AST Blast Radius recursive CTE. If >= threshold (default 3), escalates to Pro tier | `ast_blast_radius` integer count | `{"model": "gemini-3.1-pro"}` | If blast radius below threshold, remains Flash | `RESEARCH.md:130-132`, `ADR-002:12-13`, `router.py:328-331` |
| 20 | Model Router | Hot Configuration Reload | Dynamically updates tier model versions and blast radius thresholds at runtime without code deployment or restart | TOML string or JSON dict with new model mappings | Updated `tier_models` and `blast_radius_threshold` | Logs warning on malformed config keys | `router.py:118-140`, `test_router.py:99-117` |
| 21 | Model Router | Decision Audit Logging | Records audit log entries for every routing decision detailing tier, model ID, reason, tenant ID, and metadata | Routing invocation context | `RoutingDecision` instance appended to audit history and logger | Logs error if tenant ID missing | `RESEARCH.md:67`, `router.py:69-88,378-392` |
| 22 | Model Router | Tenant Budget Enforcer | Monitors tenant token and dollar spend against soft limits (emits warning) and hard limits | Usage tokens and cost increments | None (allowed) | Throws `BudgetExceededError`, blocking all new interactions | `aura-rte-domain/SKILL.md:9`, `router.py:46-66,178-185` |
| 23 | Model Router | 90% Cost Webhook & Queue Freeze | When tenant reaches 90% hard cost budget, fires synchronous webhook; suspends Zenoh queue; blocks Pro tier | Tenant usage at 90% of hard cost budget | HTTP POST to billing webhook URL; queue state change | On webhook rejection (non-200) or timeout, agent is terminated; Pro tier raises `ProTierBlockedError` | `aura-rte-domain/SKILL.md:9`, `aura-core/src/transport/webhook.rs:66-260`, `router.py:192-205` |
| 24 | Context Gap | 80% Saturation Detection | Monitors token consumption against model maximum window (1M-2M); flags saturation when >= 80% | Current token count, model limit | Boolean saturation flag | Triggers automated pause workflow | `RESEARCH.md:156-157`, `ADR-002:16-17`, `context.py:49-83` |
| 25 | Context Gap | Context Compression & Seeding | Pauses task (`context/pause`), dispatches summarization prompt to Flash-Lite, seeds new Interaction ID, resumes task | Distilled summary prompt, active session ID | `(distilled_prompt, new_seed_id)` tuple | Restores execution state on failure | `RESEARCH.md:156-157`, `context.py:199-291` |
| 26 | Context Gap | Distilled Context Formatting | Injects dense background summary into subsequent prompt instruction using `format_resumed_prompt` | Summary text and next instruction | Formatted composite prompt string | Preserves instructions cleanly | `context.py:85-88,279-281` |
| 27 | Context Gap | Infinite Summarization Guard | Enforces hard limit on consecutive summarization loops (default 3, hard limit ceiling 5 per ADR-002) | Consecutive summarization counter | Distilled prompt if under limit | Raises `InfiniteSummarizationError`; terminates task to prevent infinite spend | `ADR-002:16-17`, `context.py:44-46,214-220` |

---

## Edge Cases

| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | Credential Protection (COG-009) | Prompt containing `AIzaSyD3x94Kl09mN_OPQ8392klmnOpqRstUvwX` | Immediately raises `SecurityCredentialLeakError` ("Potential API credential detected"); outbound network request is aborted. |
| 2 | Credential Protection (COG-009) | Prompt containing `Authorization: Bearer ya29.a0AfH6SM...` | Regex detects Bearer token; immediately raises `SecurityCredentialLeakError`. |
| 3 | Tool Argument Validation (COG-010) | Required parameter missing from tool call | Raises `ToolArgumentValidationError("Missing required argument 'path' in tool invocation")`. |
| 4 | Tool Argument Validation (COG-010) | Incorrect parameter type (e.g. `lines: "ten"` when schema requires integer) | Raises `ToolArgumentValidationError("Argument 'lines' expected integer, got str")`. |
| 5 | Transient Error Retry (COG-008) | HTTP 429 ResourceExhausted or HTTP 503 Unavailable from Gemini API | Retries with jittered exponential backoff (e.g. attempt 1: ~1.2s, attempt 2: ~2.4s) up to `max_retries` before re-raising. |
| 6 | Semantic Cache (COG-013) | Exact duplicate prompt string dispatched a second time | Bypasses Gemini API completely; returns cached response in <50ms with `total_tokens: 0` and `cost_usd: 0.0`. |
| 7 | Semantic Cache (COG-013) | Remote Zenoh cache daemon unavailable or timeout | Falls back silently to direct Gemini Interactions API call without crashing the agent. |
| 8 | Model Routing (RTE-004) | Prompt text containing "conduct comprehensive literature review" without `agent_options` | Routes to Flash (or Pro if cognitive planning matches); strictly prevents Deep Research from executing unless explicitly configured in options. |
| 9 | Model Routing (RTE-005) | Refactor proposal affecting 4 reverse-dependencies (threshold = 3) | Dynamically escalates model from Flash to Pro (`gemini-3.1-pro`), logging blast radius escalation rationale in audit record. |
| 10 | Hard Cost Budget (RTE-008) | Cumulative spend exceeds `hard_cost_limit` ($11.00 spent on $10.00 budget) | Raises `BudgetExceededError`; completely blocks any new interaction turns. |
| 11 | 90% Cost Threshold Webhook (RTE-009) | Spend reaches $91 on $100 budget; billing webhook responds HTTP 402 or drops connection | Billing manager terminates agent; router raises `ProTierBlockedError` if Pro tier attempted. Flash/Flash-Lite tasks remain allowed if under hard limit. |
| 12 | 90% Cost Threshold Webhook (RTE-009) | Spend reaches 90% of budget; billing server responds HTTP 200 OK | `QueueSuspensionManager` resumes the suspended Zenoh queue; agent resumes execution normally. |
| 13 | Context Saturation (GAP-006) | Token chain reaches 805k tokens on 1M token Flash model (80.5%) | Flags saturation; emits `aura/{tenant}/context/pause` event over Zenoh; routes prompt to Flash-Lite for dense summarization; seeds `seed-<uuid>` interaction ID; emits `aura/{tenant}/context/resume`. |
| 14 | Infinite Summarization (GAP-008) | 4th consecutive summarization loop triggered without making forward progress | Raises `InfiniteSummarizationError("Hard limit of 3 consecutive summarizations exceeded")`; aborts execution to prevent catastrophic token drain. |
| 15 | Infinite Summarization Reset (GAP-008) | Task completes a normal turn after a summarization cycle | `reset_consecutive_loops()` resets counter to 0, allowing future legitimate summarizations. |

---

## Requirements Survey: Cognition (COG-001 through COG-014)

### COG-001: Gemini Interactions API Exclusive Usage
- **Full Description:** The cognitive subsystem must strictly utilize Google's Gemini Interactions API (`/v1beta/interactions`) for all agentic reasoning and interaction turns, deprecating and prohibiting legacy `generateContent` stateless endpoints.
- **Source Document:** `ROADMAP.md` Phase 3 Item 1; `RESEARCH.md` Section 53-55; `aura-cog-domain/SKILL.md` Rule 1; `05-gemini-interactions-routing.md` Section 1.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 3, 343-346 (direct invocation of `self.client.aio.interactions.create`)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 63-91 (`test_cog_001_to_003_interactions_api_and_sse_streaming`)
- **Specific Next Steps:** None required.

### COG-002: Server-Side State Management via `previous_interaction_id`
- **Full Description:** Follow-up interaction turns must pass `previous_interaction_id` instead of resending previous turn history, leveraging server-side conversation state to maximize cache hit rates, decrease TTFT, and minimize token spend.
- **Source Document:** `ROADMAP.md` Phase 3 Item 1; `RESEARCH.md` Section 56-58; `aura-cog-domain/SKILL.md` Rule 2; `05-gemini-interactions-routing.md` Section 1.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 199, 291, 330-333, 365-367 (tracks `self.previous_interaction_id`, injects into request kwargs, updates from `interaction.created` SSE frame)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 92-109 (asserts second turn passes `previous_interaction_id="sess-100"` with clean new input prompt)
- **Specific Next Steps:** None required.

### COG-003: Server-Sent Events (SSE) Streaming Ingestion
- **Full Description:** The client must consume streaming SSE events from the Interactions API and expose structured typed events (`cog.thought`, `cog.tool_call`, `cog.text_delta`, `cog.interaction_created`, `cog.interaction_completed`, `cog.error`) to agents.
- **Source Document:** `ROADMAP.md` Phase 3 Item 2; `RESEARCH.md` Section 59-60; `aura-cog-domain/SKILL.md` Rule 3; `05-gemini-interactions-routing.md` Section 1.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 30-58 (definition of `EventType`, `ToolCall`, and `CognitiveEvent` data structures), Lines 361-456 (async parsing of `interaction.created`, `step.delta` text and thought summaries, `step.start` function calls, `interaction.completed` usage)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 85-91 (asserts presence of `EventType.INTERACTION_CREATED`, `THOUGHT`, `TEXT_DELTA`, `INTERACTION_COMPLETED`)
- **Specific Next Steps:** None required.

### COG-004: Tool & Function Calling Sandbox Interception & Reinjection
- **Full Description:** Intercept `function_call` steps emitted by the model, execute the requested tool securely inside isolated WASM or Firecracker sandboxes, and reinject `function_result` payloads back into the same interaction session via `previous_interaction_id`.
- **Source Document:** `ROADMAP.md` Phase 3 Item 2; `RESEARCH.md` Section 59-60; `aura-cog-domain/SKILL.md` Rule 4; `05-gemini-interactions-routing.md` Section 1.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 458-506 (`execute_turn_with_tool_interception()` consumes SSE, intercepts `TOOL_CALL`, calls sandbox `tool_executor`, and reinjects `function_result` back with `previous_interaction_id`)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 112-156 (`test_cog_004_function_call_interception` tests interception, sandbox tool execution, and reinjection loop)
- **Specific Next Steps:** None required.

### COG-005: Long-Horizon Background Tasks (`background=true`)
- **Full Description:** Long-horizon autonomous workflows (e.g., massive monorepo refactoring, database migrations) must specify `background=true`, detaching the request from standard HTTP timeouts and tracking completion via Zenoh status topics or polling.
- **Source Document:** `ROADMAP.md` Phase 3 Item 5; `RESEARCH.md` Section 61-64; `aura-cog-domain/SKILL.md` Rule 5; `05-gemini-interactions-routing.md` Section 1.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 334-336, 347-356 (handles `background=True`, disabling stream mode and yielding `INTERACTION_CREATED`), Lines 508-537 (`await_background_status()` listening on Zenoh topic `interactions/{id}/status` with polling fallback)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 158-182 (`test_cog_005_background_execution`)
- **Specific Next Steps:** None required.

### COG-006: Native Retrieval-Augmented Generation via `file_search_stores`
- **Full Description:** Integrate native RAG using the `file_search_stores` abstraction in Gemini interaction payloads, allowing agents to pull multimodal context without requiring a host-managed external vector database.
- **Source Document:** `RESEARCH.md` Section 68-70; `aura-cog-domain/SKILL.md` Rule 6; `05-gemini-interactions-routing.md` Section 1.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 286, 313-317 (injects `file_search` tool payload with `file_search_store_names` into outbound request)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 184-206 (asserts `file_search` tool injected with configured store name)
- **Specific Next Steps:** None required.

### COG-007: Token Economics, TTFT, and Cost Metrics
- **Full Description:** Record granular metrics per interaction turn, including TTFT (Time-to-First-Token), prompt tokens, candidate tokens, cached content tokens, total tokens, and USD cost calculated according to model tier pricing tables.
- **Source Document:** `RESEARCH.md` Section 57, 73-80; `07-guardrails-token-economics.md`; `aura-sdk/aura_sdk/cognition/gemini.py`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 61-73 (`InteractionMetrics` dataclass), Lines 83-98 (`MODEL_PRICING` dictionary and `calculate_cost()` function), Lines 414-436 (calculates TTFT and appends metric to `self.metrics_history`)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 216-222 (`test_cog_007_metrics_and_pricing`)
- **Specific Next Steps:** None required.

### COG-008: Transient API Error Retry with Jittered Exponential Backoff
- **Full Description:** Automatically retry transient network and provider errors (HTTP 429 ResourceExhausted, 500, 502, 503, 504 Unavailable) using jittered exponential backoff up to a configurable maximum retry count.
- **Source Document:** `aura-sdk/aura_sdk/cognition/gemini.py`; `aura-sdk/tests/test_cognition_gemini.py`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 259-280 (`_execute_with_retry()` with exponential delay up to 30.0s + random uniform jitter)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 224-253 (`test_cog_008_transient_error_retry`)
- **Specific Next Steps:** None required.

### COG-009: API Credential Leak Prevention & Sanitization
- **Full Description:** Prevent long-lived API keys, Bearer tokens, or cloud secrets from entering the model prompt context, agent memory, or telemetry logs. Pre-flight scanning must reject leaks with `SecurityCredentialLeakError`.
- **Source Document:** `RESEARCH.md` Section 134-137; `STRIDE.md` Boundary 5 (TB5); `aura-cog-domain/SKILL.md` Rule 7.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 75-77 (`SecurityCredentialLeakError`), Lines 101-116 (`sanitize_and_check_credentials()` scanning for `AIza[0-9A-Za-z-_]{35}`, `bearer ...`, and `api_key ...`), Lines 294-295 (invoked on prompt entry)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 255-263 (`test_cog_009_credential_protection`)
- **Specific Next Steps:** None required.

### COG-010: Tool Call Argument Schema Validation
- **Full Description:** Validate model-generated tool call arguments against registered JSON Schema definitions before executing tools in sandboxes, raising `ToolArgumentValidationError` on missing required fields or type mismatches.
- **Source Document:** `aura-sdk/aura_sdk/cognition/gemini.py`; `aura-sdk/tests/test_cognition_gemini.py`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 79-81 (`ToolArgumentValidationError`), Lines 119-146 (`validate_tool_arguments()` checking required fields and types), Lines 208-210 (`register_tool_schema()`), Lines 404-407 (validation hook during `step.start` function call handling)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 265-286 (`test_cog_010_tool_schema_validation`)
- **Specific Next Steps:** None required.

### COG-011: File Search Store Deletion & Lifecycle Management
- **Full Description:** Explicitly call Gemini API deletion endpoints (`HTTP DELETE /v1beta/fileSearchStores/{name}`) upon task completion or TTL expiration, ensuring ephemeral RAG stores do not accumulate storage costs.
- **Source Document:** `aura-cog-domain/SKILL.md` Rule 6; `aura-sdk/aura_sdk/cognition/gemini.py`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 233-257 (`delete_file_search_store()` logging audit entry and calling `self.client.aio.files.delete()`)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 208-214 (verifies HTTP DELETE audit log record and store cleanup)
- **Specific Next Steps:** None required.

### COG-012: Tenant Billing Tag Injection
- **Full Description:** Inject tenant billing identifiers (`billing_tenant_id` and `x-tenant-id`) into request client metadata for all outbound interaction calls to enable downstream multi-tenant billing attribution.
- **Source Document:** `aura-sdk/aura_sdk/cognition/gemini.py`; `aura-sdk/tests/test_cognition_gemini.py`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 319-325 (`client_metadata={"billing_tenant_id": self.tenant_id, "x-tenant-id": self.tenant_id}`)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 288-306 (`test_cog_012_tenant_billing_tag_injection`)
- **Specific Next Steps:** None required.

### COG-013: Host-Side Semantic Vector Prompt Cache
- **Full Description:** SQLite vector database with `sqlite-vec` extension and BLAKE3 hashing in the Rust microkernel (`aura-core`), queried over Zenoh by Python SDK (`SemanticCacheClient`) before calling Gemini; returns exact/semantic duplicates in <50ms with 0 token spend.
- **Source Document:** `KNOWLEDGE.md` Section 70; `aura-core/src/knowledge/cache.rs`; `aura-sdk/aura_sdk/cognition/cache.py`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-core/src/knowledge/cache.rs`: Lines 79-140, 200-510 (`SemanticCache`, `sqlite-vec` schema, BLAKE3 hash, Zenoh listener)
  - `aura-sdk/aura_sdk/cognition/cache.py`: Lines 64-328 (`SemanticCacheClient` with Zenoh query/response protocol and L1 local cache)
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 148-170, 297-309 (`SemanticPromptCache` lookup in SDK)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 308-342 (`test_cog_013_semantic_cache`)
  - `aura-sdk/tests/test_router_cache_context.py`: Lines 19-56 (`test_router_semantic_cache_hit_sync`)
- **Specific Next Steps:** None required.

### COG-014: Shadow Tracker File Storage Calculations
- **Full Description:** The host-side `ShadowTracker` and SDK client must calculate cumulative Gemini API file storage costs by correlating uploaded object sizes and TTLs against configured provider pricing rates.
- **Source Document:** `ROADMAP.md` Phase 3 Item 4; `RESEARCH.md` Section 152-157; `aura-core/src/config/mod.rs`; `aura-core/src/transport/rate_limiter.rs`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-core/src/config/mod.rs`: Lines 383-485 (`PricingConfig::calculate_storage_cost_by_days()`)
  - `aura-core/src/transport/rate_limiter.rs`: Lines 66-74 (`TrackedFile`), Lines 124-176 (`ShadowTracker::register_file()`, `calculate_file_storage_costs()`)
  - `aura-core/src/transport/rate_limiter.rs`: Lines 294-328 (`test_cog_014_file_storage_costs_calculation`)
  - `aura-sdk/aura_sdk/cognition/gemini.py`: Lines 205-231 (`register_file()`, `calculate_file_storage_costs()`)
  - `aura-sdk/tests/test_cognition_gemini.py`: Lines 344-351 (`test_cog_014_file_storage_costs`)
- **Specific Next Steps:** None required.

---

## Requirements Survey: Model Router (RTE-001 through RTE-009)

### RTE-001: Four Supported Model Tiers
- **Full Description:** The model router must define and support four distinct model tiers: Pro (`gemini-3.1-pro`), Flash (`gemini-3.8-flash`), Flash-Lite (`gemini-3.5-flash-lite`), and Deep Research (`deep-research-preview`).
- **Source Document:** `ROADMAP.md` Phase 3 Item 3; `RESEARCH.md` Section 71-80; `aura-rte-domain/SKILL.md` Rule 1; `05-gemini-interactions-routing.md` Section 2.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/routing/router.py`: Lines 20-35 (`Tier` enum and `DEFAULT_TIER_MODELS`)
  - `aura-sdk/tests/test_router.py`: Lines 36-43 (`test_rte_001_four_tiers_configuration`)
- **Specific Next Steps:** None required.

### RTE-002: Default Orchestration Tier (Flash)
- **Full Description:** The router must select Flash (`gemini-3.8-flash`) as the default tier for standard software engineering workflows, feature additions, tool orchestrations, and testing.
- **Source Document:** `RESEARCH.md` Section 77; `aura-rte-domain/SKILL.md` Rule 1; `05-gemini-interactions-routing.md` Section 2.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/routing/router.py`: Line 31 (`Tier.FLASH: "gemini-3.8-flash"`), Lines 302-304 (`selected_tier = Tier.FLASH; reason = "Default orchestration tier (Flash)"`)
  - `aura-sdk/tests/test_router.py`: Lines 45-50 (`test_rte_002_default_tier_is_flash`)
- **Specific Next Steps:** None required.

### RTE-003: Cognitive Demand, Latency, and Cost Heuristics
- **Full Description:** Deterministic rule-based decision engine evaluating task requirements:
  - Route to Pro for high cognitive demand (planning, architecture, deep multi-step reasoning, or prompts > 2000 chars).
  - Route to Flash-Lite for latency constraints (< 1000ms), cost constraints (< $0.001), or lightweight validation/parsing tasks.
- **Source Document:** `ROADMAP.md` Phase 3 Item 3; `RESEARCH.md` Section 73-80; `ADR-002` Decision 2.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/routing/router.py`: Lines 332-371 (evaluates planning keywords, prompt length, `latency_budget_ms`, and `cost_budget`)
  - `aura-sdk/tests/test_router.py`: Lines 52-67 (`test_rte_003_decision_table_heuristics`)
- **Specific Next Steps:** None required.

### RTE-004: Explicit Gating for Deep Research
- **Full Description:** Deep Research (`deep-research-preview`) shall ONLY be invoked through an explicit agent option (`agent_options={"deep_research": True}` or `agent_option="deep-research"`), never autonomously selected by general keyword matching in strict mode.
- **Source Document:** `RESEARCH.md` Section 79; `aura-rte-domain/SKILL.md` Rule 1; `05-gemini-interactions-routing.md` Section 2.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/routing/router.py`: Lines 305-325, Lines 394-396 (returns `{"agent": model_id}`)
  - `aura-sdk/tests/test_router.py`: Lines 69-84 (`test_rte_004_deep_research_explicit_option_only`)
- **Specific Next Steps:** None required.

### RTE-005: AST Blast Radius Cognitive Escalation
- **Full Description:** When the proposed code modification's reverse-dependency blast radius meets or exceeds the threshold (default: 3 reverse-dependencies), the router must automatically escalate from Flash to Pro tier (`gemini-3.1-pro`).
- **Source Document:** `RESEARCH.md` Section 130-132; `ADR-002` Decision 2; `aura-rte-domain/SKILL.md` Rule 2; `05-gemini-interactions-routing.md` Section 2.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/routing/router.py`: Line 100 (`blast_radius_threshold: int = 3`), Lines 328-331 (`if ast_blast_radius >= threshold: selected_tier = Tier.PRO`)
  - `aura-sdk/tests/test_router.py`: Lines 86-97 (`test_rte_005_blast_radius_escalation`)
- **Specific Next Steps:** None required.

### RTE-006: Hot Configuration Reload
- **Full Description:** Support runtime hot reloading of tier model mappings and blast radius threshold parameters from configuration dicts or TOML strings without code changes or service restarts.
- **Source Document:** `aura-sdk/aura_sdk/routing/router.py`; `aura-sdk/tests/test_router.py`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/routing/router.py`: Lines 118-140 (`reload_config(config)`)
  - `aura-sdk/tests/test_router.py`: Lines 99-117 (`test_rte_006_hot_config_reload`)
- **Specific Next Steps:** None required.

### RTE-007: Decision Audit Logging
- **Full Description:** Every model routing decision must be logged with its selected tier, model ID, specific decision rationale, tenant ID, timestamp, and metadata for auditability.
- **Source Document:** `RESEARCH.md` Section 67; `aura-sdk/aura_sdk/routing/router.py`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/routing/router.py`: Lines 69-88 (`RoutingDecision` dataclass), Lines 378-392 (appends to `self.decision_logs`)
  - `aura-sdk/tests/test_router.py`: Lines 119-130 (`test_rte_007_decision_audit_logging`)
- **Specific Next Steps:** None required.

### RTE-008: Per-Tenant Token and Cost Budgets
- **Full Description:** Enforce per-tenant soft limits (which trigger warnings) and hard limits (which raise `BudgetExceededError`, blocking all new interactions).
- **Source Document:** `RESEARCH.md` Section 134-137; `aura-rte-domain/SKILL.md` Rule 3; `aura-sdk/aura_sdk/routing/router.py`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/routing/router.py`: Lines 37-39 (`BudgetExceededError`), Lines 46-66 (`TenantBudget`), Lines 178-185 (hard limit check)
  - `aura-sdk/tests/test_router.py`: Lines 132-146 (`test_rte_008_budget_enforcement`)
- **Specific Next Steps:** None required.

### RTE-009: 90% Cost Budget Webhook & Task Queue Suspension
- **Full Description:** When a tenant reaches 90% of their hard cost budget, trigger a synchronous HTTP billing webhook; if denied or timed out, block Pro tier interactions (`ProTierBlockedError`) in the SDK, and suspend the tenant's Zenoh queue via the host's `QueueSuspensionManager`, terminating the agent if approval fails.
- **Source Document:** `RESEARCH.md` Section 134-137; `aura-rte-domain/SKILL.md` Rule 3; `aura-core/src/config/mod.rs`; `aura-core/src/transport/rate_limiter.rs`; `aura-core/src/transport/webhook.rs`.
- **Implementation Status:** **Complete**
- **Exact Code Citations:**
  - `aura-core/src/config/mod.rs`: Lines 402-416 (`hard_cost_budget`, `budget_suspension_threshold = 0.90`, `billing_webhook_url`)
  - `aura-core/src/transport/rate_limiter.rs`: Lines 243-276 (`check_and_suspend_if_needed()`)
  - `aura-core/src/transport/webhook.rs`: Lines 66-260 (`QueueSuspensionManager`, synchronous HTTP POST with 200 OK resume or termination)
  - `aura-core/src/transport/rate_limiter.rs`: Lines 330-376 (`test_90_percent_budget_freeze_and_resume`)
  - `aura-core/src/transport/webhook.rs`: Lines 356-461 (`test_sync_billing_webhook_success_resumes_queue`, `test_sync_billing_webhook_failure_terminates_agent`, `test_sync_billing_webhook_timeout_terminates_agent`)
  - `aura-sdk/aura_sdk/routing/router.py`: Lines 41-43 (`ProTierBlockedError`), Lines 192-205 (synchronous webhook trigger at 90% budget)
  - `aura-sdk/tests/test_router.py`: Lines 148-174 (`test_rte_009_billing_webhook_and_pro_tier_blocking`)
- **Specific Next Steps:** None required.

---

## Requirements Survey: Architectural Gaps (GAP-001 through GAP-008)

### GAP-001: UDP Port 443 Middlebox & DPI Blocking
- **Full Description:** Corporate middleboxes and deep packet inspection (DPI) filter or drop UDP/QUIC traffic on port 443, risking permanent transport stalls.
- **Source Document:** `RESEARCH.md` Section 141-144; `04-mcp-http3-transport.md` Section 3; `aura-net-domain/SKILL.md`.
- **Implementation Status:** **Complete** (Phase M1 scope / Transport)
- **Exact Code Citations:**
  - `aura-core/src/transport/http3.rs`: Sans-I/O QUIC probe and fallback detection
- **Specific Next Steps:** Maintained under Phase M1 transport regression tests.

### GAP-002: 1-RTT Silent TCP Fallback via Alt-Svc / ALPN
- **Full Description:** Detect UDP failure within 1 RTT and transparently degrade MCP transport from HTTP/3 to HTTP/2 over TCP using Alt-Svc headers without dropping agent requests.
- **Source Document:** `ROADMAP.md` Phase 1 Item 3; `RESEARCH.md` Section 144-146; `04-mcp-http3-transport.md` Section 3.
- **Implementation Status:** **Complete** (Phase M1 scope / Transport)
- **Exact Code Citations:**
  - `aura-core/src/transport/http3.rs`: Alt-Svc header generation and TLS ALPN negotiation
- **Specific Next Steps:** Maintained under Phase M1 transport regression tests.

### GAP-003: WASI 0.3 Component Model Dynamic Linking Limitation
- **Full Description:** Standard WASI 0.3 lacks standardized dynamic runtime hot-linking of newly generated WASM components without pre-composition or host restart.
- **Source Document:** `RESEARCH.md` Section 147-151; `ADR-001`.
- **Implementation Status:** **Partial / Documented Architectural Boundary** (Phase M2 scope / WASM)
- **Exact Code Citations:**
  - `aura-core/src/hypervisor/wasm.rs`: Static component linker composition and pre-registered ABI imports
- **Specific Next Steps:** Await upstream WASI Component Model 1.0 specification stabilization; use host-granted WIT component composition as current workaround.

### GAP-004: Heavyweight & Dynamic Tool Execution in Ephemeral MicroVMs
- **Full Description:** Tools requiring native shell access, GUI, or complex python packages cannot execute in WASM; must run in ephemeral Firecracker microVMs wrapped by jailer with vsock streaming.
- **Source Document:** `RESEARCH.md` Section 46-52; `ROADMAP.md` Phase 2 Items 3-4; `ADR-001`.
- **Implementation Status:** **Complete** (Phase M2 scope / Sandboxes)
- **Exact Code Citations:**
  - `aura-core/src/hypervisor/firecracker.rs`: Firecracker UDS client, jailer cgroup/seccomp wrapper, ephemeral instance cleanup
- **Specific Next Steps:** Maintained under Phase M2 sandbox test suites.

### GAP-005: Context Window Saturation & State Expansion in Long-Horizon Workflows
- **Full Description:** For continuous long-horizon workflows using `background=true`, stateful conversation history in `previous_interaction_id` accumulates until exceeding model context limits (1M-2M tokens), risking catastrophic token exhaustion and cache invalidation.
- **Source Document:** `RESEARCH.md` Section 152-157; `ADR-002` Decision 4; `05-gemini-interactions-routing.md` Section 3; `aura-sdk/aura_sdk/cognition/gemini.py`: Line 3.
- **Implementation Status:** **Complete** (Phase M3 scope / Cognition)
- **Exact Code Citations:**
  - `aura-core/src/transport/rate_limiter.rs`: Lines 87-240 (`ShadowTracker` monitoring session tokens and costs)
  - `aura-sdk/aura_sdk/cognition/context.py`: Lines 49-72 (`ContextState`), Lines 123-146 (`ContextManager::record_tokens()` tracking aggregate tokens against per-model context limits)
  - `aura-sdk/tests/test_context_saturation.py`: Lines 38-61 (`test_context_manager_token_recording_and_model_limits`)
- **Specific Next Steps:** None required.

### GAP-006: 80% Saturation Detection, Task Pause, and Automated Flash-Lite Summarization
- **Full Description:** At 80% context window saturation (e.g. 800k / 1M tokens), automatically pause the active task, emit `context/pause` event over Zenoh, dispatch a summarization prompt to Flash-Lite (`gemini-3.5-flash-lite`), seed a new Interaction ID (`seed-<uuid>`), and emit `context/resume`.
- **Source Document:** `RESEARCH.md` Section 156-157; `ADR-002` Decision 4; `05-gemini-interactions-routing.md` Section 3; `aura-sdk/aura_sdk/cognition/context.py`: Lines 3, 31, 90-195, 206-291; `aura-sdk/aura_sdk/routing/router.py`: Lines 4, 258-298.
- **Implementation Status:** **Complete** (Phase M3 scope / Cognition)
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/context.py`: Line 31 (`DEFAULT_SATURATION_THRESHOLD = 0.80`), Lines 158-194 (`pause_task()`, `resume_task()` emitting Zenoh events), Lines 199-291 (`summarize_and_seed()` invoking Flash-Lite and generating `seed-<uuid>`)
  - `aura-sdk/aura_sdk/routing/router.py`: Lines 258-298 (intercepts context saturation >= 80% and routes to Flash-Lite)
  - `aura-sdk/tests/test_context_saturation.py`: Lines 24-162 (`test_saturation_threshold_detection`, `test_pause_and_resume_lifecycle_with_zenoh`, `test_summarization_with_flash_lite_and_new_seed_id`)
  - `aura-sdk/tests/test_router_cache_context.py`: Lines 58-103 (`test_router_context_saturation_routes_to_flash_lite`, `test_router_with_context_manager_instance`)
- **Specific Next Steps:** None required.

### GAP-007: Distilled Prompt Context Handoff
- **Full Description:** Prepend the dense conversation summary into the subsequent task prompt instruction (`format_resumed_prompt`) and resume execution under the newly seeded interaction session without resending the uncompressed historical chain.
- **Source Document:** `RESEARCH.md` Section 156-157; `ADR-002` Decision 4; `aura-sdk/aura_sdk/cognition/context.py`: Lines 85-88, 279-281.
- **Implementation Status:** **Complete** (Phase M3 scope / Cognition)
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/context.py`: Lines 85-88 (`format_resumed_prompt(summary, next_instruction)`), Lines 279-281 (formats distilled prompt)
  - `aura-sdk/tests/test_context_saturation.py`: Lines 151-156 (verifies prompt formatting with summary and instruction)
- **Specific Next Steps:** None required.

### GAP-008: Hard Limit Guard Against Recursive Infinite Summarization
- **Full Description:** Enforce a strict hard limit on consecutive summarization loops (configurable default 3, ceiling 5 per ADR-002) to prevent infinite loops where repeated summarization consumes tokens endlessly on circular tasks. Exceeding the limit raises `InfiniteSummarizationError` and aborts task execution. The counter is reset upon normal turn completion (`reset_consecutive_loops()`).
- **Source Document:** `ADR-002` Decision 4; `07-guardrails-token-economics.md` Section 1; `aura-sdk/aura_sdk/cognition/context.py`: Lines 32, 44-46, 195-198, 214-220.
- **Implementation Status:** **Complete** (Phase M3 scope / Cognition)
- **Exact Code Citations:**
  - `aura-sdk/aura_sdk/cognition/context.py`: Line 32 (`DEFAULT_MAX_CONSECUTIVE_SUMMARIZATIONS = 3`), Lines 44-46 (`InfiniteSummarizationError`), Lines 195-198 (`reset_consecutive_loops()`), Lines 214-220 (raises `InfiniteSummarizationError` on loop breach)
  - `aura-sdk/tests/test_context_saturation.py`: Lines 164-214 (`test_infinite_summarization_hard_limit`)
- **Specific Next Steps:** None required.

---

## Architectural Alignment Analysis

### 1. Zero-FFI Boundary & Zenoh Integration
All cognitive interactions between Python scripting (`aura-sdk`) and the Rust microkernel (`aura-core`) strictly adhere to Invariant 2 (Zero-FFI):
- The Python `SemanticCacheClient` communicates with the Rust host's `SemanticCache` over Zenoh topic `aura/{tenant_id}/cache/lookup` and replies on `aura/{tenant_id}/cache/result` / `response`.
- Context pause and resume lifecycle events are broadcast over Zenoh topics `aura/{tenant_id}/context/pause` and `aura/{tenant_id}/context/resume`.
- Background task completion is observed via Zenoh topic `interactions/{id}/status`.
- At 90% budget freeze, the Rust host's `QueueSuspensionManager` suspends the Zenoh task queue directly within the microkernel transport layer.

### 2. Multi-Tiered Routing & Blast Radius Escalation
The heuristic decision matrix in `ModelRouter` integrates directly with the Phase M4 AST Blast Radius analyzer:
- Low blast-radius changes (< 3) route to Flash (`gemini-3.8-flash`).
- Monorepo architectural modifications with high blast radius (>= 3) escalate dynamically to Pro (`gemini-3.1-pro`).
- Strict gating guarantees that Deep Research is never invoked without explicit developer authorization (`agent_options={"deep_research": True}`).

### 3. Token Economics & 90% Webhook Invariant
The dual implementation in Rust (`aura-core`) and Python (`aura-sdk`) enforces financial safety invariants:
- Storage costs for uploaded files in Gemini file search stores are continuously calculated against `PricingConfig`.
- Spending thresholds trigger synchronous webhooks before hard limits can be exceeded.
- Denial or timeout of the webhook terminates the agent and blocks Pro tier calls, preventing unexpected cloud costs.

---

## Verification Summary Table

| Category | Requirement Range | Total Requirements | Complete | Partial | Missing | Test Coverage |
|----------|-------------------|--------------------|----------|---------|---------|---------------|
| Cognition & Observability | COG-001 - COG-014 | 14 | 14 | 0 | 0 | 100% (`test_cognition_gemini.py`, `rate_limiter.rs`) |
| Model Router | RTE-001 - RTE-009 | 9 | 9 | 0 | 0 | 100% (`test_router.py`, `webhook.rs`, `rate_limiter.rs`) |
| Architectural Gaps & Mitigation | GAP-001 - GAP-008 | 8 | 7 | 1 (GAP-003)* | 0 | 100% for in-scope GAP-005 - GAP-008 (`test_context_saturation.py`, `test_router_cache_context.py`) |
| **Total** | | **31** | **30** | **1** | **0** | **96.8% Complete (100% of M3 in-scope)** |

*\*Note: GAP-003 is a known external specification boundary (WASI Component Model dynamic linking) documented in RESEARCH.md and mitigated via static WIT component linking in Phase M2.*
