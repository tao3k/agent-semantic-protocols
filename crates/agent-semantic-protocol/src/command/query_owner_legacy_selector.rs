use std::path::PathBuf;

pub(super) struct LegacyOwnerSelector {
    pub(super) owner_path: PathBuf,
    pub(super) term: String,
}

pub(super) fn parse_legacy_owner_selector(selector: &str) -> Option<LegacyOwnerSelector> {
    let (term, locator) = selector.split_once('@')?;
    let term = term.trim();
    if term.is_empty() {
        return None;
    }
    Some(LegacyOwnerSelector {
        owner_path: legacy_display_selector_owner_path(locator)?,
        term: term.to_string(),
    })
}

fn legacy_display_selector_owner_path(locator: &str) -> Option<PathBuf> {
    let mut owner = locator.trim();
    if owner.is_empty() {
        return None;
    }
    for _ in 0..2 {
        let Some((path, suffix)) = owner.rsplit_once(':') else {
            break;
        };
        if !legacy_display_selector_line_suffix(suffix) {
            break;
        }
        owner = path;
    }
    if owner.is_empty() {
        return None;
    }
    Some(PathBuf::from(owner))
}

fn legacy_display_selector_line_suffix(suffix: &str) -> bool {
    !suffix.is_empty()
        && suffix
            .split('-')
            .all(|part| !part.is_empty() && part.parse::<usize>().is_ok())
}
