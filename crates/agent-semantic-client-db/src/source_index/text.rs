//! Source-index text projection helpers for transient import assembly.

pub(super) fn source_line_count(text: &str) -> u32 {
    text.lines().count().max(1).min(u32::MAX as usize) as u32
}

pub(super) fn source_query_keys(relative_path: &str, text: &str) -> Vec<String> {
    let mut keys = Vec::new();
    push_query_key_terms(&mut keys, relative_path);
    for line in text.lines().take(64) {
        push_query_key_terms(&mut keys, line);
        if keys.len() >= 32 {
            break;
        }
    }
    keys.sort();
    keys.dedup();
    keys.truncate(32);
    keys
}

fn push_query_key_terms(keys: &mut Vec<String>, text: &str) {
    for term in text
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
        .filter(|term| term.len() >= 2)
    {
        keys.push(term.to_ascii_lowercase());
    }
}
