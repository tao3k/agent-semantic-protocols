use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCandidateSparsityInput {
    pub path: String,
    pub symbol: String,
}

impl GraphCandidateSparsityInput {
    pub fn new(path: impl Into<String>, symbol: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            symbol: symbol.into(),
        }
    }
}

pub fn select_sparse_graph_candidate_indices(
    candidates: &[GraphCandidateSparsityInput],
    limit: usize,
) -> Vec<usize> {
    let mut selected = Vec::new();
    let mut selected_indices = HashSet::new();
    let mut symbol_counts: HashMap<&str, usize> = HashMap::new();
    let mut per_symbol_limit = 1usize;
    while selected.len() < limit && selected_indices.len() < candidates.len() {
        let mut added = false;
        for (index, candidate) in candidates.iter().enumerate() {
            if selected.len() >= limit {
                break;
            }
            if selected_indices.contains(&index) {
                continue;
            }
            let symbol_count = symbol_counts
                .get(candidate.symbol.as_str())
                .copied()
                .unwrap_or(0);
            if symbol_count >= per_symbol_limit {
                continue;
            }
            selected_indices.insert(index);
            symbol_counts.insert(candidate.symbol.as_str(), symbol_count + 1);
            selected.push(index);
            added = true;
        }
        if !added {
            per_symbol_limit += 1;
        }
    }
    selected
}
