## 2024-05-18 - Caching Cedar Policies via LazyLock

**Learning:** Redundant O(N) AST parsing of hardcoded Cedar policies (`PolicySet::from_str`) inside the Tokio worker threads caused a significant evaluation bottleneck during authorization checks.
**Action:** Wrap the parsing step in a `std::sync::LazyLock` block to evaluate the policy exactly once during the microkernel lifecycle, upgrading subsequent evaluations to O(1) cached retrievals.
