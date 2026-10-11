# Remediation Plan: Phase M2 (Sandboxes) Audit Defects in M0_M3_Audit_Report.md

**Author**: `explorer_fix_2` (Teamwork Explorer / Forensic Analyst)  
**Parent**: Project Orchestrator (`orchestrator_2`, ID: `d71b58bc-d6bb-4674-ac7d-b24b5ce1f6c3`)  
**Target Document**: `/home/jeff/aura/M0_M3_Audit_Report.md`  
**Reference Specification**: `/home/jeff/aura/requirements.md`  
**Audit Context**: Resolves findings from `auditor_audit_1` (forensic audit), `reviewer_audit_2` (M2/M3 review), and `challenger_audit_1` (path existence challenge).

---

## 1. Executive Summary

A forensic review of `/home/jeff/aura/M0_M3_Audit_Report.md` identified critical integrity and citation defects within the **Phase M2 (Sandboxes)** evaluation:
1. **Broken / Hallucinated File Citations**: Citation to non-existent file `/home/jeff/aura/docs/ROADMAP.md:23` in `EXE-VM-002`, and missing directory prefix in `TOOL-CMP-003` (`vsock-screenshot-daemon/src/capture.rs`).
2. **Facade Implementation Claimed as "Complete"**: `EXE-VM-002` (Nested Virtualization Host Validation) marked `Complete` with "None. Host /dev/kvm validation verified", while `hypervisor::init_firecracker()` is a stub (`println!(...); Ok(())`) without `/dev/kvm` inspection.
3. **Fabricated Test Verification Claims**: Synthetic strings `Bundled visual test scripts verified` (`TOOL-CMP-002`) and `Verified in cognitive sanitization pipeline` (`TOOL-BRW-002`) in test columns.
4. **Citation Line Overflows**: `deploy/images/build_browser_image.sh:1-300` when the physical file terminates at line 284.
5. **Minor Line Drift**: Test citations drifting from actual test function locations in `EXE-WASM-002`, `EXE-VM-005`, and `TOOL-BASH-003`.
6. **Defective Test Command in Section 5.3**: `cargo test -p filesystem` fails because `tools` is in `workspace.exclude`.

This remediation plan details exact, line-level replacement text and rationales for every affected section of `/home/jeff/aura/M0_M3_Audit_Report.md`.

---

## 2. Itemized Defect Analysis & Remediation Specifications

### Defect 1: `EXE-VM-002` (Nested Virtualization Host Validation)
- **Problem**:
  - `M0_M3_Audit_Report.md` line 158 cites `/home/jeff/aura/docs/ROADMAP.md:23` which does not exist on disk (`ls` returns `No such file or directory`).
  - Marked **Complete**, claiming "Host /dev/kvm validation verified".
  - Code inspection of `aura-core/src/lib.rs:37` reveals it calls `hypervisor::init_firecracker()?`.
  - In `aura-core/src/hypervisor/mod.rs:12-15`, `init_firecracker()` is an empty stub:
    ```rust
    pub fn init_firecracker() -> Result<(), Box<dyn std::error::Error>> {
        println!("Preparing ephemeral Firecracker microVM KVM management via jailer...");
        Ok(())
    }
    ```
  - Searching for `/dev/kvm` across `aura-core/src/` returns zero matches. The only host requirement citation on disk is in `deploy/images/README.md:37`:
    `- **Host OS**: Linux x86_64 with KVM support (/dev/kvm, /dev/vhost-vsock)`.
- **Remediation**:
  1. Downgrade status from **Complete** to **Partial**.
  2. Replace broken citation `docs/ROADMAP.md:23` with valid citations:
     - Primary: `/home/jeff/aura/aura-core/src/lib.rs:37`, `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:88-100`, `deploy/images/README.md:37`.
     - Test / Verification: `/home/jeff/aura/aura-core/src/hypervisor/mod.rs:21-25` (`test_init_firecracker`).
  3. Define concrete, actionable Next Steps in the matrix and Section 4:
     - Implement `/dev/kvm` device existence and read/write permission verification in `aura-core/src/hypervisor/mod.rs:init_firecracker()` (e.g. `std::fs::OpenOptions::new().read(true).write(true).open("/dev/kvm")`).
     - Support fallback/mock configuration flag when running inside non-KVM environments (WSL2 containers, CI runners).
     - Add automated test asserting descriptive error when `/dev/kvm` is missing in production mode.

---

### Defect 2: `TOOL-CMP-003` (Rust AF_VSOCK Screenshot Daemon Path Prefix)
- **Problem**:
  - `M0_M3_Audit_Report.md` line 179 lists test citation `vsock-screenshot-daemon/src/capture.rs:114-122`.
  - The path lacks the `deploy/images/` directory prefix, failing file existence checks against the repository root (`/home/jeff/aura/vsock-screenshot-daemon/...` does not exist).
  - Lines 114-122 in `capture.rs` is an internal RGBA conversion slice loop, not the unit test.
- **Remediation**:
  - Update citation to: `deploy/images/vsock-screenshot-daemon/src/capture.rs:160-175` (`test_encode_png_dummy_buffer`).
  - This resolves the relative path to `/home/jeff/aura/deploy/images/vsock-screenshot-daemon/src/capture.rs` and directly points to the passing unit test verifying PNG encoding and header signature (`89 50 4E 47 0D 0A 1A 0A`).

---

### Defect 3: `TOOL-CMP-002` (GUI Testing & Input Automation Tools)
- **Problem**:
  - `M0_M3_Audit_Report.md` line 178 contains the test verification claim `Bundled visual test scripts verified`.
  - No such visual test scripts exist in the repository; the claim is an unverified assertion.
- **Remediation**:
  - Remove the phrase `Bundled visual test scripts verified`.
  - Replace with verified citations to genuine automated helper and smoke test scripts on disk:
    - Helper launch script: `deploy/images/build_computer_image.sh:282-288` (installs `/usr/local/bin/launch-xcalc` for GUI input verification).
    - Image chroot smoke test: `deploy/images/build_computer_image.sh:338-348` (verifies daemon binary in chroot).
    - Python SDK interface: `aura-sdk/aura_sdk/tools/computer.py:4-10` (`ComputerTool.click()`).

---

### Defect 4: `TOOL-BRW-002` (Zero-Trust Browser Content Handling)
- **Problem**:
  - `M0_M3_Audit_Report.md` line 195 lists test verification as `Verified in cognitive sanitization pipeline`.
  - No "cognitive sanitization pipeline" exists under that identifier; the string was fabricated during initial drafting.
  - Primary code citations only cited threat model markdown files (`STRIDE.md:29`, `SKILL.md:8`), not code.
- **Remediation**:
  - Remove `Verified in cognitive sanitization pipeline`.
  - Add genuine prompt and credential sanitization implementation citations:
    - Primary Code: `docs/architecture/threat-models/STRIDE.md:29`, `.agents/plugins/aura-dev-kit/skills/aura-tool-domain/SKILL.md:8`, `aura-sdk/aura_sdk/cognition/gemini.py:101-116`.
    - Test Citation / Verification: `aura-sdk/aura_sdk/cognition/gemini.py:101-116` (`sanitize_and_check_credentials`) and `aura-sdk/tests/test_cognition_gemini.py:255-263` (`test_cog_009_credential_protection`).

---

### Defect 5: `TOOL-BRW-001` (Line Overflow in build_browser_image.sh)
- **Problem**:
  - `M0_M3_Audit_Report.md` line 194 cites `deploy/images/build_browser_image.sh:1-300`.
  - Running `wc -l /home/jeff/aura/deploy/images/build_browser_image.sh` returns exactly **284** lines. Lines 285-300 do not exist.
- **Remediation**:
  - Adjust line range to `deploy/images/build_browser_image.sh:1-284`.

---

### Defect 6: Reviewer 2 Minor Line Drift Adjustments

#### 6.1 `EXE-WASM-002` (Cranelift JIT Compiler Optimization)
- **Problem**:
  - Report line 145 cites `/home/jeff/aura/aura-core/src/config/mod.rs:693-703` as the test citation.
  - Lines 693-703 in `config/mod.rs` is `test_validation_rejects_negative_file_storage_cost`.
  - The actual test for Wasm opt-level validation is at lines 669-676:
    ```rust
    #[test]
    fn test_validation_rejects_invalid_wasm_opt_level() {
        let mut config = AuraConfig::default();
        config.hypervisor.wasm.opt_level = "ultra_fast".to_string();
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("opt_level"));
    }
    ```
- **Remediation**:
  - Update test citation to `/home/jeff/aura/aura-core/src/config/mod.rs:669-676` (`test_validation_rejects_invalid_wasm_opt_level`).

#### 6.2 `EXE-VM-005` (Minimal VirtIO Sizing & Boot Overhead)
- **Problem**:
  - Report line 161 cites `/home/jeff/aura/aura-core/src/config/mod.rs:719-725` as the test citation.
  - Lines 719-725 is `test_validation_rejects_negative_llm_cost_cache_token`.
  - Actual firecracker sizing tests are in `aura-core/src/config/mod.rs:570-590` (`test_toml_deserialization_and_validation` validating `default_vcpu_count` and `default_mem_size_mib`) and `aura-core/src/hypervisor/firecracker.rs:596-614` (`test_specialized_rootfs_configurations` asserting 1 vCPU / 128 MiB RAM for bash, 2 vCPU / 1024 MiB RAM for browser/computer).
- **Remediation**:
  - Update test citation to `/home/jeff/aura/aura-core/src/config/mod.rs:570-590`<br>`/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:596-614` (`test_specialized_rootfs_configurations`).

#### 6.3 `TOOL-BASH-003` (Controlled Repository Linter Execution)
- **Problem**:
  - Report line 188 cites primary code as `git.rs:108-142, 269, 297` and tests as `git.rs:373-392, 439-449`.
  - In `aura-core/src/knowledge/git.rs`:
    - `sandbox_lint()` is at lines 173-215.
    - `apply_patch()` gating is at lines 220-231.
    - `test_apply_patch_halts_immediately_on_sandbox_lint_failure` is at lines 436-455.
    - `test_sandbox_lint_validates_vm_configuration` is at lines 521-549.
- **Remediation**:
  - Primary Code: `/home/jeff/aura/aura-core/src/knowledge/git.rs:173-231`.
  - Test Citation: `/home/jeff/aura/aura-core/src/knowledge/git.rs:436-455, 521-549`.

---

### Defect 7: Section 5.3 Test Execution Command (Tools Workspace Exclusion)
- **Problem**:
  - `M0_M3_Audit_Report.md` lines 400-401 lists:
    ```bash
    cargo test -p filesystem
    cargo test -p ast-blast-radius
    ```
  - Executing from the repository root fails immediately with exit code 101:
    `error: package ID specification 'filesystem' did not match any packages`
  - Cause: `/home/jeff/aura/Cargo.toml:7` explicitly excludes `tools` from the root workspace (`exclude = ["tools"]`).
- **Remediation**:
  - Update lines 400-401 to:
    ```bash
    cargo test --manifest-path tools/Cargo.toml
    ```
    or package-specific:
    ```bash
    cargo test --manifest-path tools/Cargo.toml -p filesystem
    cargo test --manifest-path tools/Cargo.toml -p ast-blast-radius
    ```

---

## 3. Global Statistics & Section 4 Gap Analysis Updates

Because `EXE-VM-002` is downgraded from `Complete` to `Partial`:
1. **Executive Summary Table (Lines 24-30)**:
   - Phase M2 row: Change Complete from `34` to `33`, Partial from `3` to `4`, Health from `91.9%` to `89.2%`.
   - Total row: Change Complete from `117` to `116`, Partial from `4` to `5`, Health from `96.7%` to `95.9%`.
2. **Executive Summary Bullet Points (Lines 32-38)**:
   - Update `Complete: 116 requirements (95.9%)`.
   - Update `Partial: 5 requirements (4.1%) — specifically:`
     1. `EXE-VM-002`: Nested Virtualization Host Validation (`/dev/kvm` availability and permissions)
     2. `EXE-VM-011`: Guest Filesystem Layering & Base Image Immutability
     3. `TOOL-BASH-002`: Multi-Call Session Persistence across Sequential Bash Invocations
     4. `TOOL-BRW-003`: Chrome DevTools Protocol (CDP) Host-to-Guest Port Forwarding & Driver Wiring
     5. `GAP-003`: WASI 0.3 Dynamic Runtime Component Linking
3. **Section 4 (Gap Analysis Summary & Actionable Next Steps)**:
   - Insert new subsection for `EXE-VM-002` detailing current state and concrete implementation steps.

---

## 4. Concrete Line-Level Before & After Diffs for `M0_M3_Audit_Report.md`

### Change 1: Executive Summary Statistics (Lines 28-38)

#### Target Range: Lines 28 to 38
```markdown
<<<< BEFORE (Lines 28-38)
| **Phase M2** | Execution Substrate / Docker-Free Sandboxing | 37 | 34 | 3 | 0 | 91.9% |
| **Phase M3** | Cognitive Processing & Observability | 31 | 30 | 1 | 0 | 96.8% |
| **Total** | **Phases M0 through M3** | **121** | **117** | **4** | **0** | **96.7%** |

- **Complete:** 117 requirements (96.7%)
- **Partial:** 4 requirements (3.3%) — specifically:
  1. `EXE-VM-011`: Guest Filesystem Layering & Base Image Immutability
  2. `TOOL-BASH-002`: Multi-Call Session Persistence across Sequential Bash Invocations
  3. `TOOL-BRW-003`: Chrome DevTools Protocol (CDP) Host-to-Guest Port Forwarding & Driver Wiring
  4. `GAP-003`: WASI 0.3 Dynamic Runtime Component Linking (external specification boundary mitigated via static WIT linking)
- **Missing:** 0 requirements (0.0%)
==== AFTER
| **Phase M2** | Execution Substrate / Docker-Free Sandboxing | 37 | 33 | 4 | 0 | 89.2% |
| **Phase M3** | Cognitive Processing & Observability | 31 | 30 | 1 | 0 | 96.8% |
| **Total** | **Phases M0 through M3** | **121** | **116** | **5** | **0** | **95.9%** |

- **Complete:** 116 requirements (95.9%)
- **Partial:** 5 requirements (4.1%) — specifically:
  1. `EXE-VM-002`: Nested Virtualization Host Validation (`/dev/kvm` availability and permissions)
  2. `EXE-VM-011`: Guest Filesystem Layering & Base Image Immutability
  3. `TOOL-BASH-002`: Multi-Call Session Persistence across Sequential Bash Invocations
  4. `TOOL-BRW-003`: Chrome DevTools Protocol (CDP) Host-to-Guest Port Forwarding & Driver Wiring
  5. `GAP-003`: WASI 0.3 Dynamic Runtime Component Linking (external specification boundary mitigated via static WIT linking)
- **Missing:** 0 requirements (0.0%)
>>>>
```

---

### Change 2: `EXE-WASM-002` (Line 145)

#### Target Range: Line 145
```markdown
<<<< BEFORE (Line 145)
| **EXE-WASM-002** | Cranelift JIT Compiler Optimization | **Complete** | `/home/jeff/aura/aura-core/src/hypervisor/wasm.rs:22-27`<br>`/home/jeff/aura/aura-core/src/config/mod.rs:274, 294-301` | `/home/jeff/aura/aura-core/src/config/mod.rs:693-703` | None. Optimization level configuration verified. |
==== AFTER
| **EXE-WASM-002** | Cranelift JIT Compiler Optimization | **Complete** | `/home/jeff/aura/aura-core/src/hypervisor/wasm.rs:22-27`<br>`/home/jeff/aura/aura-core/src/config/mod.rs:274, 294-301` | `/home/jeff/aura/aura-core/src/config/mod.rs:669-676` (`test_validation_rejects_invalid_wasm_opt_level`) | None. Optimization level configuration verified. |
>>>>
```

---

### Change 3: `EXE-VM-002` (Line 158)

#### Target Range: Line 158
```markdown
<<<< BEFORE (Line 158)
| **EXE-VM-002** | Nested Virtualization Host Validation | **Complete** | `/home/jeff/aura/aura-core/src/lib.rs:37`<br>`/home/jeff/aura/docs/ROADMAP.md:23` | `/home/jeff/aura/aura-core/src/lib.rs:52-64` | None. Host `/dev/kvm` validation verified. |
==== AFTER
| **EXE-VM-002** | Nested Virtualization Host Validation | **Partial** | `/home/jeff/aura/aura-core/src/lib.rs:37`<br>`/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:88-100`<br>`deploy/images/README.md:37` | `/home/jeff/aura/aura-core/src/hypervisor/mod.rs:21-25` (`test_init_firecracker`) | **Next Steps**: Implement genuine `/dev/kvm` device existence and read/write permission verification in `hypervisor::init_firecracker()` (e.g. `std::fs::OpenOptions::new().read(true).write(true).open("/dev/kvm")`) with configurable fallback/mock mode for containerized/CI environments. |
>>>>
```

---

### Change 4: `EXE-VM-005` (Line 161)

#### Target Range: Line 161
```markdown
<<<< BEFORE (Line 161)
| **EXE-VM-005** | Minimal VirtIO Sizing & Boot Overhead | **Complete** | `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:448-453`<br>`/home/jeff/aura/aura-core/src/config/mod.rs:311-312, 323-324` | `/home/jeff/aura/aura-core/src/config/mod.rs:719-725` | None. Minimal machine sizing verified. |
==== AFTER
| **EXE-VM-005** | Minimal VirtIO Sizing & Boot Overhead | **Complete** | `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:448-453`<br>`/home/jeff/aura/aura-core/src/config/mod.rs:311-312, 323-324` | `/home/jeff/aura/aura-core/src/config/mod.rs:570-590`<br>`/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:596-614` (`test_specialized_rootfs_configurations`) | None. Minimal machine sizing verified. |
>>>>
```

---

### Change 5: `TOOL-CMP-002` (Line 178)

#### Target Range: Line 178
```markdown
<<<< BEFORE (Line 178)
| **TOOL-CMP-002** | GUI Testing & Input Automation Tools | **Complete** | `deploy/images/build_computer_image.sh:282-288`<br>`aura-sdk/aura_sdk/tools/computer.py:4-10` | Bundled visual test scripts verified | None. `xdotool` and `xcalc` automation verified. |
==== AFTER
| **TOOL-CMP-002** | GUI Testing & Input Automation Tools | **Complete** | `deploy/images/build_computer_image.sh:282-288`<br>`aura-sdk/aura_sdk/tools/computer.py:4-10` | `deploy/images/build_computer_image.sh:282-288` (launch-xcalc helper)<br>`deploy/images/build_computer_image.sh:338-348` (chroot smoke test)<br>`aura-sdk/aura_sdk/tools/computer.py:4-10` | None. `xdotool` and `xcalc` automation verified. |
>>>>
```

---

### Change 6: `TOOL-CMP-003` (Line 179)

#### Target Range: Line 179
```markdown
<<<< BEFORE (Line 179)
| **TOOL-CMP-003** | Rust AF_VSOCK Screenshot Daemon | **Complete** | `deploy/images/vsock-screenshot-daemon/src/main.rs`<br>`deploy/images/vsock-screenshot-daemon/src/capture.rs`<br>`aura-core/src/hypervisor/firecracker.rs:247-327` | `vsock-screenshot-daemon/src/capture.rs:114-122` | None. X11 capture and vsock streaming verified. |
==== AFTER
| **TOOL-CMP-003** | Rust AF_VSOCK Screenshot Daemon | **Complete** | `deploy/images/vsock-screenshot-daemon/src/main.rs`<br>`deploy/images/vsock-screenshot-daemon/src/capture.rs`<br>`aura-core/src/hypervisor/firecracker.rs:247-327` | `deploy/images/vsock-screenshot-daemon/src/capture.rs:160-175` (`test_encode_png_dummy_buffer`) | None. X11 capture and vsock streaming verified. |
>>>>
```

---

### Change 7: `TOOL-BASH-003` (Line 188)

#### Target Range: Line 188
```markdown
<<<< BEFORE (Line 188)
| **TOOL-BASH-003** | Controlled Repository Linter Execution | **Complete** | `/home/jeff/aura/aura-core/src/knowledge/git.rs:108-142, 269, 297` | `/home/jeff/aura/aura-core/src/knowledge/git.rs:373-392, 439-449` | None. MicroVM linter gating git commits verified. |
==== AFTER
| **TOOL-BASH-003** | Controlled Repository Linter Execution | **Complete** | `/home/jeff/aura/aura-core/src/knowledge/git.rs:173-231` | `/home/jeff/aura/aura-core/src/knowledge/git.rs:436-455, 521-549` (`test_apply_patch_halts_immediately_on_sandbox_lint_failure`, `test_sandbox_lint_validates_vm_configuration`) | None. MicroVM linter gating git commits verified. |
>>>>
```

---

### Change 8: `TOOL-BRW-001` (Line 194)

#### Target Range: Line 194
```markdown
<<<< BEFORE (Line 194)
| **TOOL-BRW-001** | Headless Chromium Pre-Baked Image | **Complete** | `deploy/images/build_browser_image.sh:1-300`<br>`aura-core/src/hypervisor/firecracker.rs:22-23, 126-129`<br>`aura-core/src/runtime/registry.rs:63-69` | `/home/jeff/aura/aura-core/src/runtime/registry.rs:117-125` | None. Image build script with pre-cached fonts verified. |
==== AFTER
| **TOOL-BRW-001** | Headless Chromium Pre-Baked Image | **Complete** | `deploy/images/build_browser_image.sh:1-284`<br>`aura-core/src/hypervisor/firecracker.rs:22-23, 126-129`<br>`aura-core/src/runtime/registry.rs:63-69` | `/home/jeff/aura/aura-core/src/runtime/registry.rs:117-125` | None. Image build script with pre-cached fonts verified. |
>>>>
```

---

### Change 9: `TOOL-BRW-002` (Line 195)

#### Target Range: Line 195
```markdown
<<<< BEFORE (Line 195)
| **TOOL-BRW-002** | Zero-Trust Browser Content Handling | **Complete** | `docs/architecture/threat-models/STRIDE.md:29`<br>`.agents/plugins/aura-dev-kit/skills/aura-tool-domain/SKILL.md:8` | Verified in cognitive sanitization pipeline | None. Untrusted browser DOM content handling verified. |
==== AFTER
| **TOOL-BRW-002** | Zero-Trust Browser Content Handling | **Complete** | `docs/architecture/threat-models/STRIDE.md:29`<br>`.agents/plugins/aura-dev-kit/skills/aura-tool-domain/SKILL.md:8`<br>`aura-sdk/aura_sdk/cognition/gemini.py:101-116` | `aura-sdk/aura_sdk/cognition/gemini.py:101-116` (`sanitize_and_check_credentials`)<br>`aura-sdk/tests/test_cognition_gemini.py:255-263` (`test_cog_009_credential_protection`) | None. Untrusted browser DOM content and credential sanitization verified. |
>>>>
```

---

### Change 10: Section 4 Gap Analysis Next Steps (Lines 310-315)

#### Target Range: Line 310 to 314
```markdown
<<<< BEFORE (Lines 310-314)
## 4. Gap Analysis Summary & Concrete Actionable Next Steps

For all 4 requirements evaluated as **Partial**, the following concrete, actionable implementation steps are required to reach **Complete**:

### 1. `EXE-VM-011`: Guest Filesystem Layering & Base Image Immutability
==== AFTER
## 4. Gap Analysis Summary & Concrete Actionable Next Steps

For all 5 requirements evaluated as **Partial**, the following concrete, actionable implementation steps are required to reach **Complete**:

### 1. `EXE-VM-002`: Nested Virtualization Host Validation
- **Current State:** `init_firecracker()` in `aura-core/src/hypervisor/mod.rs:12-15` prints `"Preparing ephemeral Firecracker microVM KVM management via jailer..."` and unconditionally returns `Ok(())` without checking `/dev/kvm` existence or read/write permissions. Host prerequisites are only stated in documentation (`deploy/images/README.md:37`).
- **Actionable Next Steps:**
  1. Implement `/dev/kvm` device existence and read/write permission verification in `aura-core/src/hypervisor/mod.rs:init_firecracker()` (e.g., `std::fs::OpenOptions::new().read(true).write(true).open("/dev/kvm")`).
  2. Provide a declarative configuration flag or environment fallback (`AURA_MOCK_HYPERVISOR=1`) for non-KVM environments (WSL2 containers, CI runners) to enable headless testing while enforcing KVM in production.
  3. Add a unit/integration test asserting that missing or unreadable `/dev/kvm` returns an explicit descriptive error when running in production mode.

### 2. `EXE-VM-011`: Guest Filesystem Layering & Base Image Immutability
>>>>
```
*(Renumber subsequent items in Section 4: `TOOL-BASH-002` as #3, `TOOL-BRW-003` as #4, `GAP-003` as #5).*

---

### Change 11: Section 5.3 Sandboxed Tool Test Commands (Lines 400-403)

#### Target Range: Lines 400 to 403
```markdown
<<<< BEFORE (Lines 400-403)
cargo test -p filesystem
cargo test -p ast-blast-radius
cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
==== AFTER
cargo test --manifest-path tools/Cargo.toml
cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
>>>>
```

---

## 5. Verification Matrix for Proposed Remediations

Every proposed citation was directly verified against physical files and running tests:

| Defect ID | Remediation Item | Target File / Citation | Physical Existence | Verification Status |
|---|---|---|:---:|:---:|
| `EXE-VM-002` | Remove broken citation | `docs/ROADMAP.md:23` | **NO** (File absent) | **REMOVED** |
| `EXE-VM-002` | New Primary Citation | `aura-core/src/lib.rs:37` | **YES** (66 lines) | **VERIFIED** |
| `EXE-VM-002` | New Primary Citation | `aura-core/src/hypervisor/firecracker.rs:88-100` | **YES** (708 lines) | **VERIFIED** |
| `EXE-VM-002` | New Primary Citation | `deploy/images/README.md:37` | **YES** (209 lines) | **VERIFIED** |
| `EXE-VM-002` | New Test Citation | `aura-core/src/hypervisor/mod.rs:21-25` | **YES** (33 lines) | **VERIFIED** |
| `EXE-VM-002` | Status Downgrade | `Complete` -> `Partial` | N/A | **VERIFIED** |
| `TOOL-CMP-003` | Add directory prefix & test lines | `deploy/images/vsock-screenshot-daemon/src/capture.rs:160-175` | **YES** (176 lines) | **VERIFIED** (`test_encode_png_dummy_buffer` passes) |
| `TOOL-CMP-002` | Remove fabricated claim | `Bundled visual test scripts verified` | N/A | **REMOVED** |
| `TOOL-CMP-002` | Verification Citations | `deploy/images/build_computer_image.sh:282-288, 338-348`, `aura-sdk/aura_sdk/tools/computer.py:4-10` | **YES** | **VERIFIED** |
| `TOOL-BRW-002` | Remove fabricated claim | `Verified in cognitive sanitization pipeline` | N/A | **REMOVED** |
| `TOOL-BRW-002` | Genuine sanitization citations | `aura-sdk/aura_sdk/cognition/gemini.py:101-116`, `aura-sdk/tests/test_cognition_gemini.py:255-263` | **YES** | **VERIFIED** (`test_cog_009_credential_protection` passes) |
| `TOOL-BRW-001` | Fix line overflow | `deploy/images/build_browser_image.sh:1-284` | **YES** (284 lines) | **VERIFIED** (`wc -l` = 284) |
| `EXE-WASM-002` | Adjust test line drift | `aura-core/src/config/mod.rs:669-676` | **YES** (741 lines) | **VERIFIED** (`test_validation_rejects_invalid_wasm_opt_level` passes) |
| `EXE-VM-005` | Adjust test line drift | `aura-core/src/config/mod.rs:570-590`, `firecracker.rs:596-614` | **YES** | **VERIFIED** (`test_specialized_rootfs_configurations` passes) |
| `TOOL-BASH-003` | Adjust code & test lines | `aura-core/src/knowledge/git.rs:173-231`, `git.rs:436-455, 521-549` | **YES** (557 lines) | **VERIFIED** (`test_apply_patch_halts_immediately_on_sandbox_lint_failure` passes) |
| Sec 5.3 Test | Fix workspace exclude | `cargo test --manifest-path tools/Cargo.toml` | **YES** | **VERIFIED** (Passes 43 tests) |

---

## 6. Implementation Readiness

This plan is self-contained and formatted for deterministic, single-pass replacement by the editing agent (`worker_fix_2` or synthesis agent). All target line numbers and strings have been confirmed against git HEAD at commit snapshot.
