use ephemeral_tunnel::EphemeralTunnelService;

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
