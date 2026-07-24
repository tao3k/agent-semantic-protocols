pub(in crate::engine) const SOURCE_INDEX_READ_MODEL_MAX_QUERY_BYTES: usize = 16 * 1024;
pub(super) const SOURCE_INDEX_READ_MODEL_MAX_CANDIDATES: u32 = 256;
pub(in crate::engine) const SOURCE_INDEX_READ_MODEL_MAX_TERMS: usize = 32;

pub(super) fn source_index_read_model_terms(query: &str) -> Result<Vec<String>, String> {
    if query.len() > SOURCE_INDEX_READ_MODEL_MAX_QUERY_BYTES {
        return Err(format!(
            "source-index query exceeds byte budget: bytes={} maxBytes={SOURCE_INDEX_READ_MODEL_MAX_QUERY_BYTES}",
            query.len()
        ));
    }
    Ok(query
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
        .filter(|term| !term.is_empty())
        .take(SOURCE_INDEX_READ_MODEL_MAX_TERMS)
        .map(|term| term.to_ascii_lowercase())
        .collect())
}

pub(super) fn source_index_structured_candidate_score(
    path: &str,
    language_id: Option<&str>,
    provider_id: Option<&str>,
    source_kind: &str,
    query_keys: &[String],
    selector_haystack: &str,
    terms: &[String],
) -> usize {
    if terms.is_empty() {
        return 1;
    }
    let capacity = path.len()
        + language_id.map_or(0, str::len)
        + provider_id.map_or(0, str::len)
        + source_kind.len()
        + query_keys.iter().map(String::len).sum::<usize>()
        + selector_haystack.len()
        + query_keys.len()
        + 4;
    let mut haystack = String::with_capacity(capacity);
    haystack.push_str(path);
    haystack.push(' ');
    if let Some(language_id) = language_id {
        haystack.push_str(language_id);
        haystack.push(' ');
    }
    if let Some(provider_id) = provider_id {
        haystack.push_str(provider_id);
        haystack.push(' ');
    }
    haystack.push_str(source_kind);
    haystack.push(' ');
    for query_key in query_keys {
        haystack.push_str(query_key);
        haystack.push(' ');
    }
    haystack.push_str(selector_haystack);
    let haystack = haystack.to_lowercase();
    terms.iter().filter(|term| haystack.contains(*term)).count()
}
