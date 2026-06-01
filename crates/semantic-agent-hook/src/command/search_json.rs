use crate::protocol::{LanguageProfile, ProfileRegistry};

use super::shell::is_separator;

pub(crate) fn contains_ingest_pipe(tokens: &[String], profiles: &[&LanguageProfile]) -> bool {
    profiles.iter().any(|profile| {
        tokens.windows(3).any(|window| {
            window[0] == profile.binary && window[1] == "search" && window[2] == "ingest"
        })
    })
}

pub(crate) fn search_json_route<'a>(
    registry: &'a ProfileRegistry,
    tokens: &[String],
) -> Option<(&'a LanguageProfile, Vec<String>)> {
    for profile in &registry.profiles {
        let Some(binary_index) = tokens.iter().position(|token| token == &profile.binary) else {
            continue;
        };
        if tokens.get(binary_index + 1).map(String::as_str) != Some("search") {
            continue;
        }
        let mut argv = tokens[binary_index..]
            .iter()
            .take_while(|token| !is_separator(token))
            .filter(|token| token.as_str() != "--json")
            .cloned()
            .collect::<Vec<_>>();
        if !argv.iter().any(|arg| arg == "--view") {
            let insert_at = argv
                .iter()
                .rposition(|arg| arg == ".")
                .unwrap_or(argv.len());
            argv.splice(
                insert_at..insert_at,
                ["--view".to_string(), "seeds".to_string()],
            );
        }
        return Some((profile, argv));
    }
    None
}
