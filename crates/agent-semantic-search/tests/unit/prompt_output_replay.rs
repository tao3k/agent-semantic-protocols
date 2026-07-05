use crate::prompt_output_replay::{
    PromptOutputFingerprintRequest, PromptOutputReplayRequest, is_prime_seed_search_request,
    prompt_output_artifact_replay_safe, prompt_output_request_fingerprint,
};

#[test]
fn prompt_output_replay_rejects_obsolete_compact_graph_grammar() {
    assert!(!prompt_output_artifact_replay_safe(
        "[search-obsolete] q=GraphAlias alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
alias: graph:{G=search,Q=query}\n"
    ));
}

#[test]
fn prompt_output_replay_requires_prime_decision_primer() {
    assert!(!prompt_output_artifact_replay_safe(
        "[search-prime] q=cache\n|next search pipe cache --view seeds\n"
    ));
    assert!(prompt_output_artifact_replay_safe(
        "[search-prime] q=cache\n|decision purpose=decision-primer\n"
    ));
}

#[test]
fn prime_seed_search_request_accepts_split_or_equals_view() {
    let split = vec![
        "prime".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
    ];
    let equals = vec!["prime".to_string(), "--view=seeds".to_string()];
    let miss = vec![
        "prime".to_string(),
        "--view".to_string(),
        "hits".to_string(),
    ];

    assert!(is_prime_seed_search_request(PromptOutputReplayRequest {
        is_search_method: true,
        forwarded_args: &split,
    }));
    assert!(is_prime_seed_search_request(PromptOutputReplayRequest {
        is_search_method: true,
        forwarded_args: &equals,
    }));
    assert!(!is_prime_seed_search_request(PromptOutputReplayRequest {
        is_search_method: true,
        forwarded_args: &miss,
    }));
    assert!(!is_prime_seed_search_request(PromptOutputReplayRequest {
        is_search_method: false,
        forwarded_args: &split,
    }));
}

#[test]
fn prompt_output_fingerprint_includes_prime_render_abi() {
    let args = vec!["prime".to_string(), "--view=seeds".to_string()];
    let prime = prompt_output_request_fingerprint(PromptOutputFingerprintRequest {
        language_id: "rust",
        provider_id: "rs-harness",
        normalized_project_root: "/repo",
        export_method: "search/prime",
        forwarded_args: &args,
    });
    let lexical = prompt_output_request_fingerprint(PromptOutputFingerprintRequest {
        language_id: "rust",
        provider_id: "rs-harness",
        normalized_project_root: "/repo",
        export_method: "search/lexical",
        forwarded_args: &args,
    });

    assert!(prime.starts_with("fnv64:"));
    assert_ne!(prime, lexical);
}
