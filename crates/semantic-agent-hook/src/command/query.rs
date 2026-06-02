use crate::protocol::{DecisionRoute, DecisionRouteKind};
use crate::protocol_activation::ActivatedProvider;
use crate::source_selector::provider_source_selector;

pub(crate) fn infer_query_from_path(path: &str) -> Option<String> {
    let normalized = path.trim().trim_end_matches('/');
    let file_name = normalized.rsplit('/').next()?;
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    let base = if matches!(stem, "index" | "mod" | "__init__") {
        normalized.rsplit('/').nth(1).unwrap_or(stem)
    } else {
        stem
    };
    query_variants(base)
}

pub(crate) fn search_query_route(
    provider: &ActivatedProvider,
    terms: &[String],
) -> Option<DecisionRoute> {
    let selector = provider_source_selector(provider);
    let template = provider
        .routes
        .query
        .as_ref()
        .unwrap_or(&provider.routes.fzf);
    let argv = template
        .argv
        .iter()
        .flat_map(|arg| expand_query_arg(arg, &selector, terms))
        .collect();
    let argv = apply_provider_command_prefix(provider, argv);
    Some(DecisionRoute {
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        binary: provider.binary.clone(),
        kind: DecisionRouteKind::Query,
        argv,
        stdin_mode: template.stdin_mode,
    })
}

fn apply_provider_command_prefix(provider: &ActivatedProvider, argv: Vec<String>) -> Vec<String> {
    if !provider.provider_command_prefix.is_empty()
        && argv
            .first()
            .is_some_and(|command| command == &provider.binary)
    {
        return provider
            .provider_command_prefix
            .iter()
            .cloned()
            .chain(argv.into_iter().skip(1))
            .collect();
    }
    argv
}

fn expand_query_arg(arg: &str, selector: &str, terms: &[String]) -> Vec<String> {
    if arg == "{termArgs}" {
        return terms
            .iter()
            .flat_map(|term| ["--term".to_string(), term.clone()])
            .collect();
    }
    let query = terms.join(",");
    vec![
        arg.replace("{selector}", selector)
            .replace("{query}", &query)
            .replace("{projectRoot}", "."),
    ]
}

fn query_variants(base: &str) -> Option<String> {
    let raw = base.trim_matches(|ch: char| !ch.is_ascii_alphanumeric());
    if raw.is_empty() {
        return None;
    }
    let pascal = title_case_identifier(raw);
    let camel = lower_first_ascii(&pascal);
    let mut variants = Vec::new();
    push_unique(&mut variants, raw.to_string());
    if !pascal.is_empty() {
        push_unique(&mut variants, pascal);
    }
    if !camel.is_empty() {
        push_unique(&mut variants, camel);
    }
    Some(variants.join("|"))
}

fn title_case_identifier(value: &str) -> String {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(uppercase_first_ascii)
        .collect::<String>()
}

fn uppercase_first_ascii(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut output = String::new();
    output.push(first.to_ascii_uppercase());
    output.extend(chars);
    output
}

fn lower_first_ascii(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut output = String::new();
    output.push(first.to_ascii_lowercase());
    output.extend(chars);
    output
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
