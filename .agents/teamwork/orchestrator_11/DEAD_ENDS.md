# Dead Ends — Phase M5 (Hardening)

| Iteration | Approach Tried | Why It Failed | Files Touched |
|-----------|---------------|---------------|---------------|
| M1 Iteration 1 | Deriving compiler-default `Debug` on `TokenMetadata` and relying only on `redacted_token()` method; allowing 8-char tokens to pass `chars.len() >= 8` | Leaked raw secrets on standard formatters `{:?}` and `{:#?}`; emitted 100% of raw secret characters for 8-char tokens in `[REDACTED-TOKEN-12345678]` | `tools/ephemeral-tunnel/src/credentials.rs` |
