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
        "entryNodeIds",
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

/// Bind the exact admitted workspace and generation identities carried by a
/// complete-generation graph payload onto the Python Graphs service envelope.
pub fn bind_graph_generation_identity(request: &mut serde_json::Value) -> Result<(), String> {
    let identity = request
        .get("payload")
        .and_then(|payload| payload.get("identity"))
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "generation graph payload lacks identity".to_owned())?;
    let workspace_identity = identity
        .get("workspaceId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "generation graph payload lacks identity.workspaceId".to_owned())?
        .to_owned();
    let generation_digest = identity
        .get("generationCandidateDigest")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "generation graph payload lacks identity.generationCandidateDigest".to_owned()
        })?
        .to_owned();
    request["workspaceIdentity"] = serde_json::Value::String(workspace_identity);
    request["generationDigest"] = serde_json::Value::String(generation_digest);
    Ok(())
}

/// Reject a response replayed from any other request, workspace, generation,
/// or lifecycle state before exposing its payload to Search.
pub fn validate_graph_generation_receipt_identity(
    receipt: &serde_json::Value,
    request_id: &str,
    state: &str,
    workspace_identity: &str,
    generation_digest: &str,
) -> Result<(), String> {
    let receipt_state = receipt
        .get("payload")
        .and_then(serde_json::Value::as_object)
        .and_then(|payload| payload.get("state"))
        .and_then(serde_json::Value::as_str);
    if receipt.get("requestId").and_then(serde_json::Value::as_str) != Some(request_id)
        || receipt_state != Some(state)
        || receipt
            .get("workspaceIdentity")
            .and_then(serde_json::Value::as_str)
            != Some(workspace_identity)
        || receipt
            .get("generationDigest")
            .and_then(serde_json::Value::as_str)
            != Some(generation_digest)
    {
        return Err(format!(
            "asp-python-graphs generation receipt identity mismatch: receipt={receipt}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bind_graph_generation_identity, validate_graph_generation_receipt_identity};

    #[test]
    fn generation_identity_is_bound_from_the_exact_graph_payload() {
        let mut request = serde_json::json!({
            "payload": {
                "identity": {
                    "workspaceId": "workspace-a",
                    "generationCandidateDigest": "blake3-256:abc"
                }
            }
        });

        bind_graph_generation_identity(&mut request).unwrap();

        assert_eq!(request["workspaceIdentity"], "workspace-a");
        assert_eq!(request["generationDigest"], "blake3-256:abc");
    }

    #[test]
    fn generation_receipt_rejects_cross_generation_replay() {
        let receipt = serde_json::json!({
            "requestId": "request-a",
            "payload": { "state": "completed" },
            "workspaceIdentity": "workspace-a",
            "generationDigest": "blake3-256:old"
        });

        assert!(
            validate_graph_generation_receipt_identity(
                &receipt,
                "request-a",
                "completed",
                "workspace-a",
                "blake3-256:new",
            )
            .is_err()
        );
    }
}
