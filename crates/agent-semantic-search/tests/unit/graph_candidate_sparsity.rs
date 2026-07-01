use crate::{GraphCandidateSparsityInput, select_sparse_graph_candidate_indices};

#[test]
fn sparse_graph_candidate_selection_retains_generated_paths_without_path_policy() {
    let candidates = vec![
        GraphCandidateSparsityInput::new("src/generated/lib.rs", "HookDecision"),
        GraphCandidateSparsityInput::new("src/domain/model.rs", "ClientReceipt"),
    ];

    let selected = select_sparse_graph_candidate_indices(&candidates, 8);

    assert_eq!(selected, vec![0, 1]);
}

#[test]
fn sparse_graph_candidate_selection_spreads_symbols_before_repeating() {
    let candidates = vec![
        GraphCandidateSparsityInput::new("src/a.rs", "Repeated"),
        GraphCandidateSparsityInput::new("src/b.rs", "Repeated"),
        GraphCandidateSparsityInput::new("src/c.rs", "Distinct"),
    ];

    let selected = select_sparse_graph_candidate_indices(&candidates, 3);

    assert_eq!(selected, vec![0, 2, 1]);
}
