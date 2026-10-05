// Built-in Cedar Policy for Aura CLI Progressive Authorization
permit (
    principal == Aura::Workload::"spiffe://aura.local/workload/aura-cli",
    action in [Aura::Action::"ReadFile", Aura::Action::"QueryIndex"],
    resource matches Aura::Workspace::"*"
);

permit (
    principal == Aura::Workload::"spiffe://aura.local/workload/aura-cli",
    action in [Aura::Action::"WriteFile", Aura::Action::"ExecuteBash"],
    resource matches Aura::Workspace::"*"
) when {
    context.permission_mode == "always-proceed"
};

forbid (
    principal,
    action in [Aura::Action::"ExecuteBash", Aura::Action::"WriteFile"],
    resource
) when {
    context.permission_mode != "always-proceed" &&
    !context.human_explicitly_approved
};
