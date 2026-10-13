# Progress — challenger_m1_1

Last visited: 2026-10-11T18:11:45Z

## Status
- Empirical adversarial stress tests executed.
- Verified concurrent minting and revocation under multi-threaded contention (16 threads, 1600 operations, 10-thread double revocation race).
- Discovered 3 empirical vulnerabilities / spec violations:
  1. `TokenMetadata` leaks raw token under `Debug` and `{:#?}` (derived Debug instead of custom redaction; does not implement Display).
  2. `Credentials` and `TokenMetadata` leak raw token under Serde serialization.
  3. Tokens of length 8 leak 100% of secret bytes in redacted handle `[REDACTED-TOKEN-12345678]`.
- Cleaned up temporary test harness file.
- Verified `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml` passes (10 unit/integration tests + 1 doc test).
- Writing final handoff report with explicit REJECT verdict.
