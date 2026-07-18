use crate::ClientDbSourceIndexImport;

#[derive(serde::Serialize)]
struct TursoSourceIndexCanonicalSelectorFact {
    selector_id: String,
    symbol: Option<String>,
    kind: Option<String>,
    start_line: u32,
    end_line: u32,
    source: String,
    payload_kind: Option<String>,
    payload_bounded: bool,
    query_keys: Vec<String>,
}

const TURSO_SOURCE_INDEX_POSTING_TERMS_PER_OWNER: usize = 16;

fn turso_source_index_terms(
    value: &str,
    terms: &mut Vec<String>,
    seen: &mut std::collections::BTreeSet<String>,
) {
    for token in value
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
        .filter(|token| !token.is_empty())
    {
        let token = token.to_ascii_lowercase();
        if seen.insert(token.clone()) {
            terms.push(token.clone());
        }
        for component in token
            .split(['_', '-'])
            .filter(|component| !component.is_empty())
        {
            let component = component.to_string();
            if seen.insert(component.clone()) {
                terms.push(component);
            }
        }
    }
}

pub(super) fn turso_source_index_canonical_selectors_by_owner(
    import: &ClientDbSourceIndexImport,
    membership: &std::collections::HashMap<&str, &str>,
    changed_owner_paths: &std::collections::BTreeSet<&str>,
) -> Result<std::collections::BTreeMap<String, (String, i64, Vec<String>)>, String> {
    let mut selectors_by_owner = std::collections::BTreeMap::<
        String,
        std::collections::BTreeMap<String, TursoSourceIndexCanonicalSelectorFact>,
    >::new();
    for selector in &import.selectors {
        let owner_path = selector.owner_path.as_str();
        if !membership.contains_key(owner_path) {
            return Err(format!(
                "source-index selector has no owner file hash: owner_path={owner_path}"
            ));
        }
        if !changed_owner_paths.contains(owner_path) {
            continue;
        }
        let selector_id = selector.selector_id.as_str().to_string();
        selectors_by_owner
            .entry(owner_path.to_string())
            .or_default()
            .entry(selector_id.clone())
            .or_insert_with(|| TursoSourceIndexCanonicalSelectorFact {
                selector_id,
                symbol: selector.symbol.clone(),
                kind: selector.kind.clone(),
                start_line: selector.start_line,
                end_line: selector.end_line,
                source: selector.source.as_str().to_string(),
                payload_kind: selector
                    .payload_proof
                    .as_ref()
                    .map(|proof| proof.payload_kind.as_str().to_string()),
                payload_bounded: selector
                    .payload_proof
                    .as_ref()
                    .is_some_and(|proof| proof.bounded),
                query_keys: selector
                    .query_keys
                    .iter()
                    .map(|key| key.as_str().to_string())
                    .collect(),
            });
    }

    import
        .owners
        .iter()
        .filter(|owner| changed_owner_paths.contains(owner.owner_path.as_str()))
        .map(|owner| {
            let selectors = selectors_by_owner
                .remove(owner.owner_path.as_str())
                .unwrap_or_default()
                .into_values()
                .collect::<Vec<_>>();
            let selector_count = selectors.len().min(i64::MAX as usize) as i64;
            let selector_facts_json = serde_json::to_string(&selectors).map_err(|error| {
                format!("failed to encode Turso source-index canonical selectors: {error}")
            })?;
            let mut terms = Vec::new();
            let mut seen_terms = std::collections::BTreeSet::new();
            for query_key in &owner.query_keys {
                turso_source_index_terms(query_key.as_str(), &mut terms, &mut seen_terms);
            }
            for selector in &selectors {
                for value in [selector.symbol.as_deref().unwrap_or_default()] {
                    turso_source_index_terms(value, &mut terms, &mut seen_terms);
                }
                for query_key in &selector.query_keys {
                    turso_source_index_terms(query_key, &mut terms, &mut seen_terms);
                }
            }
            for value in [
                owner.owner_path.as_str(),
                owner
                    .language_id
                    .as_ref()
                    .map_or("", |value| value.as_str()),
                owner
                    .provider_id
                    .as_ref()
                    .map_or("", |value| value.as_str()),
                owner.source_kind.as_str(),
            ] {
                turso_source_index_terms(value, &mut terms, &mut seen_terms);
            }
            for selector in &selectors {
                for value in [
                    selector.selector_id.as_str(),
                    selector.kind.as_deref().unwrap_or_default(),
                    selector.source.as_str(),
                ] {
                    turso_source_index_terms(value, &mut terms, &mut seen_terms);
                }
            }
            terms.truncate(TURSO_SOURCE_INDEX_POSTING_TERMS_PER_OWNER);
            Ok((
                owner.owner_path.as_str().to_string(),
                (selector_facts_json, selector_count, terms),
            ))
        })
        .collect()
}
