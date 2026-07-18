use std::fmt::Write;
use std::fs;
use std::path::Path;

use super::search::OwnerItemMatch;

pub(in crate::dynamic_search) fn render_frontier(
    language_id: &str,
    display_owner: &str,
    query: &str,
    matches: &[OwnerItemMatch],
) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "[search-owner] q={query} owner={display_owner} selector=items alg=asp-dynamic-owner-items-v1"
    );
    let language = if language_id.is_empty() {
        "code"
    } else {
        language_id
    };
    for (index, item_match) in matches.iter().enumerate() {
        render_match(&mut rendered, language, display_owner, item_match, index);
    }
    render_next_action(
        &mut rendered,
        language,
        display_owner,
        query,
        matches.first(),
    );
    rendered.push_str("entries=owner-query(O,Q=>dynamic-items)\n");
    rendered
}

pub(in crate::dynamic_search) fn display_path(locator_root: &Path, path: &Path) -> String {
    path.strip_prefix(locator_root)
        .unwrap_or(path)
        .to_string_lossy()
        .trim_start_matches("./")
        .to_string()
}

pub(in crate::dynamic_search) fn render_code(
    language_id: &str,
    display_owner: &str,
    owner_path: &Path,
    matches: &[OwnerItemMatch],
) -> String {
    let Ok(source) = fs::read_to_string(owner_path) else {
        return String::new();
    };
    let lines = source.lines().collect::<Vec<_>>();
    let language = if language_id.is_empty() {
        "code"
    } else {
        language_id
    };
    let mut rendered = String::new();
    for (index, item_match) in matches.iter().enumerate() {
        if index > 0 {
            rendered.push('\n');
        }
        let selector = structural_selector(language, display_owner, item_match);
        let _ = writeln!(
            rendered,
            "// selector={} displayLineRange={}:{} source=dynamic-owner-items",
            selector, item_match.start, item_match.end
        );
        for line_number in item_match.start..=item_match.end {
            if let Some(line) = lines.get(line_number.saturating_sub(1)) {
                let _ = writeln!(rendered, "{line}");
            }
        }
    }
    rendered
}

fn render_match(
    rendered: &mut String,
    language: &str,
    display_owner: &str,
    item_match: &OwnerItemMatch,
    index: usize,
) {
    let label = if index == 0 {
        "I".to_string()
    } else {
        format!("I{}", index + 1)
    };
    let selector = structural_selector(language, display_owner, item_match);
    let source_locator_hint = format!("{}:{}:{}", display_owner, item_match.start, item_match.end);
    let reason = if item_match.rank > 0 {
        "owner-local-source-attribution"
    } else {
        "dynamic-owner-item-ready"
    };
    let _ = writeln!(
        rendered,
        "{label}=item:symbol({})@{}!syntax",
        item_match.term, selector
    );
    let _ = writeln!(
        rendered,
        "|item symbol={} kind={} structuralSelector={} displayLineRange={}:{} sourceLocatorHint={} reason={}",
        item_match.term,
        item_match.kind,
        selector,
        item_match.start,
        item_match.end,
        source_locator_hint,
        reason,
    );
}

fn render_next_action(
    rendered: &mut String,
    language: &str,
    display_owner: &str,
    _query: &str,
    first_match: Option<&OwnerItemMatch>,
) {
    if let Some(item_match) = first_match {
        let selector = structural_selector(language, display_owner, item_match);
        let reason = if item_match.rank > 0 {
            "owner-local-source-attribution"
        } else {
            "dynamic-owner-item-ready"
        };
        let _ = writeln!(
            rendered,
            "nextCommand=asp {language} query --from-hook item-skeleton --selector {} --workspace . --names-only",
            shell_arg(&selector)
        );
        let _ = writeln!(rendered, "reason={reason}");
        rendered
            .push_str("avoid=selector-code-before-exact,direct-source-read,manual-window-scan\n");
    } else {
        rendered.push_str("recommendedNext=revise-query\n");
        rendered.push_str("actionFrontier=revise-query\n");
        rendered.push_str("reason=no-owner-item-match\n");
    }
}

fn structural_selector(language: &str, display_owner: &str, item_match: &OwnerItemMatch) -> String {
    format!(
        "{}://{}#item/{}/{}",
        language,
        display_owner,
        item_match.kind,
        item_match.term.replace(char::is_whitespace, "-")
    )
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
