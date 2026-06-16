use super::types::ClientDbSourceIndexQueryKey;

pub(super) struct SourceIndexSearchProjection {
    pub(super) query_keys_json: String,
    pub(super) search_text: String,
}

pub(super) fn source_index_search_projection<'a>(
    fixed_terms: impl IntoIterator<Item = &'a str>,
    query_keys: &'a [ClientDbSourceIndexQueryKey],
) -> Result<SourceIndexSearchProjection, String> {
    let mut search_text = String::new();
    let query_key_values = source_index_search_text(&mut search_text, fixed_terms, query_keys);
    let query_keys_json = serde_json::to_string(&query_key_values)
        .map_err(|error| format!("failed to serialize source index query keys: {error}"))?;
    Ok(SourceIndexSearchProjection {
        query_keys_json,
        search_text,
    })
}

pub(super) fn parse_query_keys(
    json: &str,
    column: usize,
) -> rusqlite::Result<Vec<ClientDbSourceIndexQueryKey>> {
    serde_json::from_str::<Vec<String>>(json)
        .map(|values| {
            values
                .into_iter()
                .map(ClientDbSourceIndexQueryKey::new)
                .collect()
        })
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

fn source_index_search_text<'a>(
    search_text: &mut String,
    fixed_terms: impl IntoIterator<Item = &'a str>,
    query_keys: &'a [ClientDbSourceIndexQueryKey],
) -> Vec<&'a str> {
    for term in fixed_terms.into_iter().filter(|term| !term.is_empty()) {
        append_search_text_term(search_text, term);
    }
    let mut query_key_values = Vec::with_capacity(query_keys.len());
    for query_key in query_keys {
        let value = query_key.as_str();
        query_key_values.push(value);
        append_search_text_term(search_text, value);
    }
    query_key_values
}

fn append_search_text_term(search_text: &mut String, term: &str) {
    if term.is_empty() {
        return;
    }
    if !search_text.is_empty() {
        search_text.push('\n');
    }
    let start = search_text.len();
    search_text.push_str(term);
    search_text[start..].make_ascii_lowercase();
}

pub(super) fn source_index_like_query(query: &str) -> String {
    let escaped = query
        .to_ascii_lowercase()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

pub(super) fn usize_to_i64(value: usize) -> i64 {
    value.min(i64::MAX as usize) as i64
}

pub(super) fn u32_to_i64(value: u32) -> i64 {
    i64::from(value)
}
