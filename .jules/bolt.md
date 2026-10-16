## 2023-10-27 - Cache Cedar PolicySet Parsing
**Learning:** Parsing Cedar policies from strings natively inside high-frequency authorization paths (e.g. `PolicySet::from_str`) causes expensive repetitive O(N) AST parsing overhead that severely bottlenecks throughput.
**Action:** Always wrap static Cedar policy sets or complex declarative structures in a `std::sync::LazyLock` to parse them exactly once during the application lifecycle.
