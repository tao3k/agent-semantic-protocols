//! Shared projection from Runtime Search requests to the Python graph service protocol.

/// Adapt the shared Runtime Search request into the bounded Python graph service payload.
pub fn adapt_graph_evaluate_payload(
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "asp-python-graphs graph request must be an object".to_owned())?;
    let terms = object
        .get("queryTerms")
        .cloned()
        .ok_or_else(|| "asp-python-graphs graph request requires queryTerms".to_owned())?;
    let mut rank_payload = serde_json::Map::new();
    for field in [
        "seedIds",
        "kindBudgets",
        "windowMerge",
        "pathBudget",
        "pathMaxHops",
        "cache",
        "queryClauses",
        "budget",
    ] {
        if let Some(value) = object.get(field) {
            rank_payload.insert(field.to_owned(), value.clone());
        }
    }
    Ok(serde_json::json!({
        "terms": terms,
        "profile": object.get("profile").cloned().unwrap_or_else(|| serde_json::Value::String("owner-query".to_owned())),
        "budget": object.get("budget").cloned().unwrap_or(serde_json::Value::from(8)),
        "rankPayload": rank_payload,
    }))
}

/// Require the graph request to bind the admitted source Merkle root exactly.
pub fn validate_graph_source_root(
    payload: &serde_json::Value,
    source_root_digest: &str,
) -> Result<(), String> {
    let root = payload
        .get("workspaceGeneration")
        .and_then(|generation| generation.get("rootDigest"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            "asp-python-graphs graph request requires workspaceGeneration.rootDigest".to_owned()
        })?;
    let matches = root == source_root_digest
        || source_root_digest
            .strip_prefix("blake3-256:")
            .is_some_and(|digest| digest == root);
    if !matches {
        return Err(format!(
            "state=stale-generation reasonKind=graph-source-root-mismatch expected={source_root_digest} observed={root}"
        ));
    }
    Ok(())
}
