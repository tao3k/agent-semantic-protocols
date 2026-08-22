pub(super) fn structured_document_format(
    candidate: &std::path::Path,
) -> Option<agent_semantic_config::HookClientStructuredFormat> {
    candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| match extension.to_ascii_lowercase().as_str() {
            "json" => Some(agent_semantic_config::HookClientStructuredFormat::Json),
            "toml" => Some(agent_semantic_config::HookClientStructuredFormat::Toml),
            _ => None,
        })
}

pub(super) fn fast_path_token(token: &str) -> Option<&str> {
    if token.starts_with('-') {
        return None;
    }
    let trimmed = token.trim_matches(|ch| matches!(ch, '"' | '\'' | ',' | ';'));
    let path = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    path.contains('.').then_some(path)
}

pub(super) fn path_without_line_range(path: &str) -> Option<&str> {
    let (base, suffix) = path.rsplit_once(':')?;
    if suffix.chars().all(|character| character.is_ascii_digit()) {
        let (base, start) = base.rsplit_once(':')?;
        return start
            .chars()
            .all(|character| character.is_ascii_digit())
            .then_some(base);
    }
    let (start, end) = suffix.split_once('-')?;
    (!start.is_empty()
        && !end.is_empty()
        && start.chars().all(|character| character.is_ascii_digit())
        && end.chars().all(|character| character.is_ascii_digit()))
    .then_some(base)
}
