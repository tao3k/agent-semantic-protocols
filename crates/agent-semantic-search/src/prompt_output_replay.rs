//! Prompt-output replay fingerprinting and safety rules.

/// Request facts needed for prompt-output replay decisions.
pub struct PromptOutputReplayRequest<'a> {
    pub is_search_method: bool,
    pub forwarded_args: &'a [String],
}

/// Input facts needed to build a prompt-output request fingerprint.
pub struct PromptOutputFingerprintRequest<'a> {
    pub language_id: &'a str,
    pub provider_id: &'a str,
    pub normalized_project_root: &'a str,
    pub export_method: &'a str,
    pub forwarded_args: &'a [String],
}

/// Return whether a prompt-output stdout artifact is safe to replay.
pub fn prompt_output_artifact_replay_safe(stdout: &str) -> bool {
    if stdout.starts_with("[search-prime]") && !stdout.contains("|decision purpose=decision-primer")
    {
        return false;
    }
    !stdout.contains("alias: graph:{")
        && !stdout
            .contains("legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next")
}

/// Return whether the request is `search prime --view seeds`.
pub fn is_prime_seed_search_request(request: PromptOutputReplayRequest<'_>) -> bool {
    request.is_search_method
        && request
            .forwarded_args
            .first()
            .is_some_and(|arg| arg == "prime")
        && (request
            .forwarded_args
            .windows(2)
            .any(|window| window[0] == "--view" && window[1] == "seeds")
            || request
                .forwarded_args
                .iter()
                .any(|arg| arg == "--view=seeds"))
}

/// Build the prompt-output request fingerprint used by client cache replay.
pub fn prompt_output_request_fingerprint(request: PromptOutputFingerprintRequest<'_>) -> String {
    let prompt_output_provenance = prompt_output_render_abi_provenance(request.export_method);
    let seed = format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}",
        request.language_id,
        request.provider_id,
        request.normalized_project_root,
        request.export_method,
        request.forwarded_args.join("\0"),
        "syntax-query-ast-abi:none",
        prompt_output_provenance
    );
    format!("fnv64:{}", stable_hash_hex(&seed))
}

fn prompt_output_render_abi_provenance(export_method: &str) -> String {
    if matches!(export_method, "search/prime" | "search/package") {
        return format!(
            "prompt-output-render-abi:fnv64:{}",
            stable_hash_hex(PRIME_DECISION_PRIMER_RENDER_ABI)
        );
    }
    "prompt-output-render-abi:none".to_string()
}

fn stable_hash_hex(value: &str) -> String {
    stable_hash_bytes(value.as_bytes())
}

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

const PRIME_DECISION_PRIMER_RENDER_ABI: &str = concat!(
    "semantic-search-prime;",
    "purpose=decision-primer;",
    "answer=false;",
    "code=false;",
    "capabilities=pipe,lexical,fd-query,rg-query,owner-items,selector-code,treesitter-query;",
    "ladder=pipe>lexical>fd-query|rg-query>owner-items>selector-code;",
    "history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath;",
    "risk=broad-direct-read,manual-window-scan,repeat-prime;",
    "next=search pipe <question-or-feature-term> --view seeds"
);
