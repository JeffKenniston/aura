use std::collections::{HashMap, VecDeque};

/// Represents a caller dependency discovered during recursive CTE traversal (TOOL-BR-002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReverseDependency {
    pub symbol_id: String,
    pub min_depth: usize,
    pub is_direct: bool,
}

/// In-memory graph dependency analyzer implementing the Recursive CTE query semantics.
/// In M4 this maps directly to SQLite:
///   WITH RECURSIVE reverse_deps(source_id, target_id, depth) AS (
///       SELECT caller_id, callee_id, 1 FROM call_graph WHERE callee_id = :symbol_id
///       UNION
///       SELECT cg.caller_id, cg.callee_id, rd.depth + 1
///       FROM call_graph cg
///       JOIN reverse_deps rd ON cg.callee_id = rd.source_id
///       WHERE rd.depth < :max_depth
///   )
///   SELECT DISTINCT source_id, min(depth) FROM reverse_deps GROUP BY source_id;
#[derive(Clone, Debug, Default)]
pub struct CallGraph {
    /// Mapping of callee -> list of callers (reverse edges)
    reverse_edges: HashMap<String, Vec<String>>,
}

impl CallGraph {
    /// Creates a new empty `CallGraph`.
    pub fn new() -> Self {
        Self {
            reverse_edges: HashMap::new(),
        }
    }

    /// Adds a caller -> callee call dependency edge.
    pub fn add_call_edge(&mut self, caller_id: &str, callee_id: &str) {
        self.reverse_edges
            .entry(callee_id.to_string())
            .or_default()
            .push(caller_id.to_string());
    }

    /// Computes reverse dependencies using recursive CTE traversal up to `max_depth` (TOOL-BR-002).
    pub fn compute_reverse_dependencies(
        &self,
        target_symbol_id: &str,
        max_depth: usize,
    ) -> Vec<ReverseDependency> {
        let mut min_depths: HashMap<String, usize> = HashMap::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        // Base case (depth 1): direct callers of target_symbol_id
        if let Some(direct_callers) = self.reverse_edges.get(target_symbol_id) {
            for caller in direct_callers {
                if caller != target_symbol_id && !min_depths.contains_key(caller) {
                    min_depths.insert(caller.clone(), 1);
                    queue.push_back((caller.clone(), 1));
                }
            }
        }

        // Recursive CTE step
        while let Some((curr_caller, curr_depth)) = queue.pop_front() {
            if curr_depth >= max_depth {
                continue;
            }

            let next_depth = curr_depth + 1;
            if let Some(upstream_callers) = self.reverse_edges.get(&curr_caller) {
                for upstream in upstream_callers {
                    if upstream == target_symbol_id {
                        continue;
                    }
                    // Update shortest depth or visit for the first time (cycle-safe)
                    match min_depths.get_mut(upstream) {
                        Some(existing_depth) => {
                            if next_depth < *existing_depth {
                                *existing_depth = next_depth;
                                queue.push_back((upstream.clone(), next_depth));
                            }
                        }
                        None => {
                            min_depths.insert(upstream.clone(), next_depth);
                            queue.push_back((upstream.clone(), next_depth));
                        }
                    }
                }
            }
        }

        // Sort by ascending depth then alphabetical symbol_id
        let mut results: Vec<ReverseDependency> = min_depths
            .into_iter()
            .map(|(symbol_id, min_depth)| ReverseDependency {
                is_direct: min_depth == 1,
                min_depth,
                symbol_id,
            })
            .collect();

        results.sort_by(|a, b| a.min_depth.cmp(&b.min_depth).then(a.symbol_id.cmp(&b.symbol_id)));
        results
    }
}
