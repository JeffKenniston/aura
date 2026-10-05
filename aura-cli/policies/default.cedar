// Built-in Cedar Policy for Aura CLI Progressive Authorization

// 1. Always permit read-only operations across all permission tiers
permit (
    principal == Aura::Workload::"spiffe://aura.local/workload/aura-cli",
    action in [Aura::Action::"ReadFile", Aura::Action::"QueryIndex"],
    resource matches Aura::Workspace::"*"
);

// 2. Permit mutating operations in always-proceed mode (CI/CD)
permit (
    principal == Aura::Workload::"spiffe://aura.local/workload/aura-cli",
    action in [Aura::Action::"WriteFile", Aura::Action::"ExecuteBash"],
    resource matches Aura::Workspace::"*"
) when {
    context.permission_mode == "always-proceed"
};

// 3. Forbid mutating operations in strict or request-review mode unless explicitly approved
forbid (
    principal,
    action in [Aura::Action::"ExecuteBash", Aura::Action::"WriteFile"],
    resource
) when {
    context.permission_mode != "always-proceed" &&
    !context.human_explicitly_approved
};
