# Dispatch for Worker M2_2 (Replacement)

You are `worker_m2_2` (role: `teamwork_preview_worker`).
Your assigned working directory is `/home/jeff/aura/.agents/teamwork/worker_m2_2/`.

## Mandatory Reading:
- User Request: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
- Master Baseline: `/home/jeff/aura/M0_M3_Audit_Report.md` (§ HOST-011, NET-007)
- Scope: `/home/jeff/aura/.agents/teamwork/orchestrator_7/PROJECT.md`
- Implementation Survey: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/handoff.md` (§ 1.1, 2.1, 3.1)
- Repository Constitution: `/home/jeff/aura/AGENTS.md` (Strict zero mock/stub policy)
- Domain Skills:
  - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-host-domain/SKILL.md`
  - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-net-domain/SKILL.md`

## Milestone 2 Tasks:
### 1. HOST-011: Declarative Component Registry TOML Manifest Loading
Target File: `aura-core/src/runtime/registry.rs`
- Remove the mock parsing logic (`if path.contains("test") { reg.tools.insert("mock_tool"...) }`).
- Implement authentic Serde deserialization for declarative TOML tool manifests:
  - Define `ToolManifestFile` with `tools: HashMap<String, ToolEntryConfig>`.
  - Define `ToolEntryConfig` enum with `#[serde(tag = "substrate", rename_all = "lowercase")]`:
    - `Wasm { path: PathBuf }`
    - `Firecracker { kernel_path: String, rootfs_path: String, vcpu_count: Option<u32>, mem_size_mib: Option<u32> }` (with serde aliases `kernel`, `rootfs`).
  - Update `KernelConfig` and `RuntimeBackend` with Serialize/Deserialize derives.
  - Implement `Registry::load_from_config<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>>` reading file from disk.
  - Implement `Registry::load_from_str(raw: &str) -> Result<Self, Box<dyn std::error::Error>>`.
  - Implement `Registry::from_manifest(manifest: ToolManifestFile) -> Self`.
  - Implement `Registry::validate_file_paths(&self) -> Result<(), RegistryError>`.
  - Write unit tests in `registry.rs`:
    - `test_registry_load_valid_toml`: Parses a valid TOML defining BOTH WASM and Firecracker tool backends, verifies resolution of tools.
    - `test_registry_load_invalid_toml`: Asserts deserialization error on malformed TOML.
    - `test_registry_validate_file_paths`: Verifies error propagation when referenced files do not exist.

### 2. NET-007: Stateless July 2026 Model Context Protocol (MCP) Server Interface
Target File: `aura-core/src/transport/mcp.rs` & `aura-core/src/transport/mod.rs`
- Implement a stateless JSON-RPC 2.0 Model Context Protocol server interface:
  - JSON-RPC 2.0 structures: `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`.
  - Supported methods:
    - `tools/list`: Enumerate registered tools from `Arc<Registry>`, returning tool names and schemas/substrates.
    - `tools/call`: Handle tool execution requests (e.g. tool name, arguments), validate existence in `Registry`, and format standard MCP response.
  - Standard JSON-RPC 2.0 error handling (-32700 Parse error, -32600 Invalid request, -32601 Method not found, -32602 Invalid params, -32603 Internal error).
  - Export module in `aura-core/src/transport/mod.rs`.
  - Integration/unit tests in `mcp.rs`:
    - `test_mcp_tools_list_jsonrpc`: Verify `tools/list` returns registered tools in compliant JSON-RPC 2.0 format.
    - `test_mcp_tools_call_success`: Verify `tools/call` for registered tool returns success.
    - `test_mcp_method_not_found`: Verify -32601 returned for unknown methods.
    - `test_mcp_invalid_jsonrpc`: Verify -32600 returned for malformed requests.

### 3. Build & Test Verification:
- Run `cargo check --all-targets`
- Run `cargo test -p aura-core` (all tests must pass with 0 failures)
- Ensure zero warnings, zero mocks, zero stubs in production code.

Write complete results and test logs in `/home/jeff/aura/.agents/teamwork/worker_m2_2/handoff.md`.

## Mandatory Integrity Warning:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.
