// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Derives compact, generation-bound qualification facts from a Search settlement.

use super::protocol_model::WorkspaceSearchQualificationReceipt;

pub(super) fn qualification_result_string(
    result: &serde_json::Value,
    field: &str,
) -> Result<String, String> {
    result
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("Live Corpus callable projection omitted {field}"))
}

pub(super) fn workspace_search_qualification_receipt(
    settlement: &serde_json::Value,
    operation_id: String,
    elapsed_micros: u64,
    response_decode_elapsed_micros: u64,
) -> Result<WorkspaceSearchQualificationReceipt, String> {
    let result_state = required_text(settlement, "resultState")?;
    let inference = settlement
        .get("inference")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "Search settlement has no inference object".to_owned())?;
    let inference_state = inference
        .get("state")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Search settlement inference has no state".to_owned())?;
    let terminal = settlement
        .get("terminal")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "Search settlement has no terminal object".to_owned())?;
    let terminal_state = terminal
        .get("state")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Search settlement terminal has no state".to_owned())?;
    let terminal_count = terminal
        .get("terminalCount")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "Search settlement terminal has no terminalCount".to_owned())?;
    let binding = settlement
        .get("binding")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "Search settlement has no binding".to_owned())?;
    let binding_digest = |field: &str| {
        binding
            .get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("Search settlement binding has no {field}"))
    };
    let selectors = settlement
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|node| node.get("selector").and_then(serde_json::Value::as_str))
        .map(str::to_owned)
        .fold(
            (std::collections::BTreeSet::new(), Vec::new()),
            |(mut seen, mut ordered), selector| {
                if seen.insert(selector.clone()) {
                    ordered.push(selector);
                }
                (seen, ordered)
            },
        )
        .1;
    let owner_paths = selectors
        .iter()
        .map(|selector| {
            agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(
                selector,
            )
            .and_then(|selector| selector.owner_path())
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .fold(
            (std::collections::BTreeSet::new(), Vec::new()),
            |(mut seen, mut ordered), owner_path| {
                if seen.insert(owner_path.clone()) {
                    ordered.push(owner_path);
                }
                (seen, ordered)
            },
        )
        .1;
    let state_matches_payload = match result_state {
        "queryable" => !selectors.is_empty(),
        "empty" => selectors.is_empty(),
        _ => false,
    };
    if inference_state != "complete"
        || terminal_state != "ready"
        || terminal_count != 1
        || !state_matches_payload
    {
        return Err(format!(
            "reasonKind=live-corpus-search-settlement-not-complete resultState={result_state} inferenceState={inference_state} terminalState={terminal_state} terminalCount={terminal_count} selectorCount={}",
            selectors.len()
        ));
    }
    Ok(WorkspaceSearchQualificationReceipt {
        operation_id,
        source_generation_digest: binding_digest("sourceGenerationDigest")?,
        provider_catalog_digest: binding_digest("providerCatalogDigest")?,
        topology_generation_digest: binding_digest("topologyGenerationDigest")?,
        selectors,
        owner_paths,
        elapsed_micros,
        response_decode_elapsed_micros,
        packet_bytes: serde_json::to_vec(settlement)
            .map_err(|error| format!("encode admitted Search settlement profile: {error}"))?
            .len(),
        node_count: settlement_array_len(settlement, "nodes")?,
        edge_count: settlement_array_len(settlement, "edges")?,
        frontier_count: settlement_array_len(settlement, "frontiers")?,
        coverage_certificate_count: settlement_array_len(settlement, "coverageCertificates")?,
    })
}

fn required_text<'a>(value: &'a serde_json::Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Search settlement has no {field}"))
}

fn settlement_array_len(settlement: &serde_json::Value, field: &str) -> Result<usize, String> {
    settlement
        .get(field)
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| format!("Search settlement has no {field} array"))
}
