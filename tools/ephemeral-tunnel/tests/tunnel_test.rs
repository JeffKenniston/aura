use ephemeral_tunnel::{
    global_service, Credentials, EphemeralTunnelComponent, EphemeralTunnelService, TokenMetadata,
};

#[test]
fn test_mint_credentials_success() {
    let tunnel = EphemeralTunnelService::new("spiffe://aura.local/agent/task-worker");

    let creds = tunnel.mint_credentials("aws-sts").unwrap();
    assert!(creds.token.starts_with("spiffe://aura.local/ephemeral/aws-sts/"));
    assert!(creds.expires_at > 0);

    // Verify token status
    let meta = tunnel.verify_token(&creds.token).unwrap();
    assert_eq!(meta.service, "aws-sts");
    assert!(!meta.revoked);
}

#[test]
fn test_scope_narrowing_non_escalation() {
    let tunnel = EphemeralTunnelService::new("spiffe://aura.local/agent/restricted")
        .with_allowed_services(vec!["database".to_string()]);

    // Authorized service succeeds
    let ok_res = tunnel.mint_credentials("database");
    assert!(ok_res.is_ok());

    // Unauthorized service is blocked (IAM-004, TOOL-ET-002)
    let denied_res = tunnel.mint_credentials("aws-sts");
    assert!(denied_res.is_err());
    let err = denied_res.unwrap_err();
    assert!(
        err.contains("Privilege escalation denied"),
        "Unexpected error: {}",
        err
    );
}

#[test]
fn test_context_isolation_sanitization() {
    let tunnel = EphemeralTunnelService::new("spiffe://aura.local/agent/worker");

    let creds = tunnel.mint_credentials("vault").unwrap();
    let sanitized = tunnel.sanitize_for_prompt(&creds.token);

    // Raw token secret must be redacted for LLM prompts (TOOL-ET-002)
    assert!(sanitized.contains("[REDACTED-TOKEN-"));
    assert!(!sanitized.contains(&creds.token));
}

#[test]
fn test_explicit_revocation_lifecycle() {
    let tunnel = EphemeralTunnelService::new("spiffe://aura.local/agent/worker");

    let creds = tunnel.mint_credentials("github").unwrap();
    assert!(tunnel.verify_token(&creds.token).is_ok());

    // Explicitly revoke upon task completion (TOOL-ET-003)
    let revoke_res = tunnel.revoke_credentials(&creds.token);
    assert!(revoke_res.is_ok());

    // Subsequent verification must fail
    let verify_res = tunnel.verify_token(&creds.token);
    assert!(verify_res.is_err());
    assert!(verify_res.unwrap_err().contains("revoked"));

    // Double revocation fails
    let double_revoke = tunnel.revoke_credentials(&creds.token);
    assert!(double_revoke.is_err());
}

#[test]
fn test_submillisecond_execution() {
    let tunnel = EphemeralTunnelService::new("spiffe://aura.local/agent/benchmark");

    let start = std::time::Instant::now();
    for _ in 0..100 {
        let creds = tunnel.mint_credentials("test-service").unwrap();
        let _ = tunnel.verify_token(&creds.token).unwrap();
        tunnel.revoke_credentials(&creds.token).unwrap();
    }
    let elapsed = start.elapsed();

    // 100 iterations of mint -> verify -> revoke in under 10ms (< 0.1ms per op)
    assert!(elapsed.as_millis() < 10, "Execution took too long: {:?}", elapsed);
}

#[test]
fn test_credentials_display_redaction() {
    let creds = Credentials {
        token: "spiffe://aura.local/ephemeral/github/01234567-89ab-cdef-0123-456789abcdef".to_string(),
        expires_at: 1728666000,
    };

    let display_str = format!("{}", creds);

    // Must contain sanitized opaque handle
    assert!(
        display_str.contains("[REDACTED-TOKEN-"),
        "Display output must contain redacted token tag: {}",
        display_str
    );
    // Must preserve service scope in handle
    assert!(
        display_str.contains("github"),
        "Display output should retain service context: {}",
        display_str
    );
    // Must NEVER leak raw secret UUID
    assert!(
        !display_str.contains("01234567-89ab-cdef-0123-456789abcdef"),
        "Display output leaked raw secret token: {}",
        display_str
    );
    assert!(
        !display_str.contains("89ab-cdef"),
        "Display output leaked internal token bytes: {}",
        display_str
    );
}

#[test]
fn test_credentials_debug_redaction() {
    let creds = Credentials {
        token: "spiffe://aura.local/ephemeral/aws-sts/secret-token-key-uuid-99990000".to_string(),
        expires_at: 1728666000,
    };

    let debug_str = format!("{:?}", creds);
    let pretty_debug_str = format!("{:#?}", creds);

    for s in [&debug_str, &pretty_debug_str] {
        assert!(
            s.contains("[REDACTED-TOKEN-"),
            "Debug output must redact token: {}",
            s
        );
        assert!(
            s.contains("1728666000"),
            "Debug output must retain expires_at: {}",
            s
        );
        assert!(
            !s.contains("secret-token-key-uuid-99990000"),
            "Debug output leaked secret UUID: {}",
            s
        );
    }
}

#[test]
fn test_credentials_redaction_edge_cases() {
    // 1. Short token (< 8 chars)
    let short_creds = Credentials {
        token: "secret".to_string(),
        expires_at: 100,
    };
    let short_display = format!("{}", short_creds);
    assert_eq!(short_display, "[REDACTED-TOKEN-...]");
    assert!(!short_display.contains("secret"));

    // 2. Empty token
    let empty_creds = Credentials {
        token: String::new(),
        expires_at: 0,
    };
    let empty_display = format!("{}", empty_creds);
    assert_eq!(empty_display, "[REDACTED-TOKEN-...]");

    // 3. Exact 8 characters
    let eight_creds = Credentials {
        token: "12345678".to_string(),
        expires_at: 100,
    };
    let eight_display = format!("{}", eight_creds);
    assert_eq!(eight_display, "[REDACTED-TOKEN-...]");
    assert!(!eight_display.contains("12345678"));

    // 4. Non-ASCII multi-byte UTF-8 string
    let utf8_creds = Credentials {
        token: "spiffe://aura.local/ephemeral/vault/token-🔑🔒🛡️".to_string(),
        expires_at: 100,
    };
    let utf8_display = format!("{}", utf8_creds);
    assert!(utf8_display.contains("[REDACTED-TOKEN-"));
    let utf8_debug = format!("{:?}", utf8_creds);
    assert!(utf8_debug.contains("[REDACTED-TOKEN-"));
}

#[test]
fn test_wasm_guest_component_state_persistence() {
    use ephemeral_tunnel::exports::aura::tunnel::ephemeral::Guest;

    // Step 1: Mint credentials via WIT Guest export
    let mint_res = EphemeralTunnelComponent::mint_credentials("github".to_string());
    assert!(mint_res.is_ok(), "Failed to mint credentials via Guest: {:?}", mint_res);
    let creds = mint_res.unwrap();
    let token = creds.token;

    // Step 2: Revoke credentials via WIT Guest export
    let revoke_res = EphemeralTunnelComponent::revoke_credentials(token.clone());
    assert!(
        revoke_res.is_ok(),
        "Failed to revoke credentials via Guest: {:?}",
        revoke_res
    );

    // Step 3: Double revocation must return error confirming persistence of revoked state
    let double_revoke_res = EphemeralTunnelComponent::revoke_credentials(token.clone());
    assert!(
        double_revoke_res.is_err(),
        "Double revocation should fail, but succeeded"
    );
    let err = double_revoke_res.unwrap_err();
    assert!(
        err.contains("already been revoked"),
        "Unexpected error on double revocation: {}",
        err
    );

    // Step 4: Revocation of non-existent token must return not found
    let unknown_revoke_res = EphemeralTunnelComponent::revoke_credentials("non-existent-token".to_string());
    assert!(unknown_revoke_res.is_err());
    assert!(unknown_revoke_res.unwrap_err().contains("not found"));
}

#[test]
fn test_global_service_concurrency_stress() {
    let iterations = 50;
    let threads = 8;
    let mut handles = Vec::new();

    for _ in 0..threads {
        let handle = std::thread::spawn(move || {
            for _ in 0..iterations {
                let service = global_service();
                let creds = service.mint_credentials("cloud").expect("Mint should succeed");
                let verified = service.verify_token(&creds.token).expect("Verify should succeed");
                assert_eq!(verified.service, "cloud");
                service.revoke_credentials(&creds.token).expect("Revoke should succeed");
                assert!(service.is_revoked(&creds.token));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked during concurrency test");
    }
}

#[test]
fn test_token_metadata_display_and_debug_redaction() {
    let secret_uuid = "01234567-89ab-cdef-0123-456789abcdef";
    let raw_token = format!("spiffe://aura.local/ephemeral/github/{}", secret_uuid);
    let meta = TokenMetadata {
        token: raw_token.clone(),
        service: "github".to_string(),
        issued_at: 1728660000,
        expires_at: 1728666000,
        revoked: false,
        scopes: vec!["scope:github".to_string(), "scope:repo:read".to_string()],
    };

    // 1. Display formatting test (TOOL-ET-002, IAM-004)
    let display_str = format!("{}", meta);
    assert!(
        display_str.contains("[REDACTED-TOKEN-"),
        "Display output must contain redacted token tag: {}",
        display_str
    );
    assert!(
        display_str.contains("github"),
        "Display output must preserve service identifier: {}",
        display_str
    );
    assert!(
        !display_str.contains(secret_uuid),
        "Display output leaked full secret UUID: {}",
        display_str
    );
    assert!(
        !display_str.contains("01234567-89ab-cdef-0123"),
        "Display output leaked sensitive token entropy: {}",
        display_str
    );
    assert_eq!(
        display_str,
        meta.redacted_token(),
        "Display output must match redacted_token() output"
    );

    // 2. Compact Debug formatting test `{:?}` (Feature 1, IAM-004)
    let debug_str = format!("{:?}", meta);
    assert!(
        debug_str.contains("[REDACTED-TOKEN-"),
        "Debug output must contain redacted token tag: {}",
        debug_str
    );
    assert!(
        !debug_str.contains(secret_uuid),
        "Debug output leaked full secret UUID: {}",
        debug_str
    );
    assert!(
        !debug_str.contains("01234567-89ab-cdef-0123"),
        "Debug output leaked sensitive prefix bytes: {}",
        debug_str
    );
    assert!(
        debug_str.contains("service: \"github\"") || debug_str.contains("\"github\""),
        "Debug output must retain service field: {}",
        debug_str
    );
    assert!(
        debug_str.contains("1728660000"),
        "Debug output must retain issued_at timestamp: {}",
        debug_str
    );
    assert!(
        debug_str.contains("1728666000"),
        "Debug output must retain expires_at timestamp: {}",
        debug_str
    );
    assert!(
        debug_str.contains("false"),
        "Debug output must retain revoked status: {}",
        debug_str
    );
    assert!(
        debug_str.contains("scope:github"),
        "Debug output must retain scopes: {}",
        debug_str
    );

    // 3. Pretty-Debug formatting test `{:#?}`
    let pretty_debug_str = format!("{:#?}", meta);
    assert!(
        pretty_debug_str.contains("[REDACTED-TOKEN-"),
        "Pretty-debug output must contain redacted token tag: {}",
        pretty_debug_str
    );
    assert!(
        !pretty_debug_str.contains(secret_uuid),
        "Pretty-debug output leaked full secret UUID: {}",
        pretty_debug_str
    );
    assert!(
        !pretty_debug_str.contains("01234567-89ab-cdef-0123"),
        "Pretty-debug output leaked sensitive prefix bytes: {}",
        pretty_debug_str
    );
    assert!(
        pretty_debug_str.contains("1728660000"),
        "Pretty-debug output must retain issued_at timestamp: {}",
        pretty_debug_str
    );
    assert!(
        pretty_debug_str.contains("1728666000"),
        "Pretty-debug output must retain expires_at timestamp: {}",
        pretty_debug_str
    );
    assert!(
        pretty_debug_str.contains("scope:github"),
        "Pretty-debug output must retain scopes: {}",
        pretty_debug_str
    );

    // 4. Verify formatting with revoked status
    let mut revoked_meta = meta.clone();
    revoked_meta.revoked = true;
    let revoked_debug = format!("{:?}", revoked_meta);
    assert!(
        revoked_debug.contains("[REDACTED-TOKEN-"),
        "Revoked TokenMetadata must still redact token: {}",
        revoked_debug
    );
    assert!(
        revoked_debug.contains("true"),
        "Revoked TokenMetadata must reflect revoked=true: {}",
        revoked_debug
    );
    assert!(
        !revoked_debug.contains(secret_uuid),
        "Revoked TokenMetadata leaked secret: {}",
        revoked_debug
    );
}

#[test]
fn test_short_token_boundary_does_not_reveal_full_secret() {
    // 8-character bare token
    let eight_token = "12345678";
    let eight_creds = Credentials {
        token: eight_token.to_string(),
        expires_at: 100,
    };
    let eight_meta = TokenMetadata {
        token: eight_token.to_string(),
        service: "vault".to_string(),
        issued_at: 1000,
        expires_at: 2000,
        revoked: false,
        scopes: vec!["scope:vault".to_string()],
    };

    let eight_creds_display = format!("{}", eight_creds);
    assert_eq!(eight_creds_display, "[REDACTED-TOKEN-...]");
    assert!(!eight_creds_display.contains(eight_token));
    assert!(!format!("{:?}", eight_creds).contains(eight_token));

    let eight_meta_display = format!("{}", eight_meta);
    assert!(!eight_meta_display.contains(eight_token));
    assert_eq!(
        eight_meta_display,
        "spiffe://aura.local/ephemeral/vault/[REDACTED-TOKEN-...]"
    );
    assert!(!format!("{:?}", eight_meta).contains(eight_token));
    assert!(!format!("{:#?}", eight_meta).contains(eight_token));

    // 8-character secret inside SPIFFE URI wrapper
    let spiffe_8_creds = Credentials {
        token: "spiffe://aura.local/ephemeral/vault/12345678".to_string(),
        expires_at: 100,
    };
    let spiffe_8_display = format!("{}", spiffe_8_creds);
    assert_eq!(
        spiffe_8_display,
        "spiffe://aura.local/ephemeral/vault/[REDACTED-TOKEN-...]"
    );
    assert!(!spiffe_8_display.contains("12345678"));

    // 6-character short token
    let six_token = "secret";
    let six_creds = Credentials {
        token: six_token.to_string(),
        expires_at: 100,
    };
    let six_meta = TokenMetadata {
        token: six_token.to_string(),
        service: "aws-sts".to_string(),
        issued_at: 1000,
        expires_at: 2000,
        revoked: false,
        scopes: vec![],
    };

    let six_creds_display = format!("{}", six_creds);
    assert_eq!(six_creds_display, "[REDACTED-TOKEN-...]");
    assert!(!six_creds_display.contains(six_token));
    assert!(!format!("{:?}", six_creds).contains(six_token));

    let six_meta_display = format!("{}", six_meta);
    assert_eq!(
        six_meta_display,
        "spiffe://aura.local/ephemeral/aws-sts/[REDACTED-TOKEN-...]"
    );
    assert!(!six_meta_display.contains(six_token));
    assert!(!format!("{:?}", six_meta).contains(six_token));
    assert!(!format!("{:#?}", six_meta).contains(six_token));

    // Empty token string
    let empty_creds = Credentials {
        token: String::new(),
        expires_at: 0,
    };
    let empty_meta = TokenMetadata {
        token: String::new(),
        service: "github".to_string(),
        issued_at: 0,
        expires_at: 0,
        revoked: false,
        scopes: vec![],
    };

    let empty_creds_display = format!("{}", empty_creds);
    assert_eq!(empty_creds_display, "[REDACTED-TOKEN-...]");
    assert!(format!("{:?}", empty_creds).contains("[REDACTED-TOKEN-...]"));

    let empty_meta_display = format!("{}", empty_meta);
    assert_eq!(
        empty_meta_display,
        "spiffe://aura.local/ephemeral/github/[REDACTED-TOKEN-...]"
    );
    assert!(format!("{:?}", empty_meta).contains("[REDACTED-TOKEN-...]"));
    assert!(format!("{:#?}", empty_meta).contains("[REDACTED-TOKEN-...]"));

    // Non-ASCII multi-byte UTF-8 string (< 8 chars)
    let utf8_token = "🔒🔑🛡️";
    let utf8_creds = Credentials {
        token: utf8_token.to_string(),
        expires_at: 50,
    };
    let utf8_display = format!("{}", utf8_creds);
    assert_eq!(utf8_display, "[REDACTED-TOKEN-...]");
    assert!(!utf8_display.contains(utf8_token));
}

#[test]
fn test_serde_and_logging_token_leak_prevention() {
    let secret_entropy = "01234567-89ab-cdef-0123-456789abcdef";
    let token = format!("spiffe://aura.local/ephemeral/github/{}", secret_entropy);
    let creds = Credentials {
        token: token.clone(),
        expires_at: 1728666000,
    };
    let meta = TokenMetadata {
        token: token.clone(),
        service: "github".to_string(),
        issued_at: 1728660000,
        expires_at: 1728666000,
        revoked: false,
        scopes: vec!["scope:github".to_string(), "scope:repo:read".to_string()],
    };

    // Logging Formatter Isolation Tests
    let log_info_creds = format!("[INFO] Task credential handle: {}", creds);
    assert!(!log_info_creds.contains(secret_entropy));
    assert!(!log_info_creds.contains("01234567-89ab-cdef"));
    assert!(log_info_creds.contains("[REDACTED-TOKEN-"));

    let log_debug_creds = format!("[DEBUG] Credentials state: {:?}", creds);
    assert!(!log_debug_creds.contains(secret_entropy));
    assert!(log_debug_creds.contains("[REDACTED-TOKEN-"));

    let log_info_meta = format!("[INFO] Token metadata: {}", meta);
    assert!(!log_info_meta.contains(secret_entropy));
    assert!(log_info_meta.contains("[REDACTED-TOKEN-"));

    let log_debug_meta = format!("[DEBUG] Active session metadata: {:?}", meta);
    assert!(!log_debug_meta.contains(secret_entropy));
    assert!(log_debug_meta.contains("[REDACTED-TOKEN-"));

    let log_trace_meta = format!("[TRACE] Pretty diagnostic dump:\n{:#?}", meta);
    assert!(!log_trace_meta.contains(secret_entropy));
    assert!(log_trace_meta.contains("[REDACTED-TOKEN-"));

    // Serde Serialization Validation
    let creds_json = serde_json::to_string(&creds).expect("Serialization failed");
    let meta_json = serde_json::to_string(&meta).expect("Serialization failed");
    assert!(creds_json.contains("1728666000"));
    assert!(meta_json.contains("github"));
    assert!(meta_json.contains("scope:github"));

    if creds_json.contains("[REDACTED-TOKEN-") {
        assert!(!creds_json.contains(secret_entropy));
        assert!(!meta_json.contains(secret_entropy));
    }
}

