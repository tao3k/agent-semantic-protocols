use crate::protocol::{DecisionRoute, DecisionRouteKind};
use crate::protocol_activation::protocol_activation_manifest::ActivatedProvider;

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

pub(crate) fn selector_query_route(provider: &ActivatedProvider, path: &str) -> DecisionRoute {
    let route_context = provider.route_path_context(path);
    let output_flag = if is_document_provider(provider) {
        "--content"
    } else {
        "--code"
    };
    DecisionRoute {
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        binary: "asp".to_string(),
        kind: DecisionRouteKind::Query,
        argv: provider.agent_facade_argv([
            "query",
            "--selector",
            route_context.selector.as_str(),
            "--workspace",
            route_context.project_root.as_str(),
            output_flag,
        ]),
        stdin_mode: None,
    }
}

fn is_document_provider(provider: &ActivatedProvider) -> bool {
    matches!(provider.language_id.as_str(), "org" | "md")
}

pub(crate) fn search_query_route_for_selector(
    provider: &ActivatedProvider,
    selector: &str,
    project_root: &str,
    terms: &[String],
) -> Option<DecisionRoute> {
    let (kind, template) = if let Some(template) = provider.routes.query.as_ref() {
        (DecisionRouteKind::Query, template)
    } else {
        (DecisionRouteKind::Lexical, &provider.routes.lexical)
    };
    Some(route_from_query_template(
        provider,
        kind,
        template,
        selector,
        project_root,
        terms,
    ))
}

fn route_from_query_template(
    provider: &ActivatedProvider,
    kind: DecisionRouteKind,
    template: &crate::protocol::CommandTemplate,
    selector: &str,
    project_root: &str,
    terms: &[String],
) -> DecisionRoute {
    let argv = template
        .argv
        .iter()
        .flat_map(|arg| expand_query_arg(arg, selector, project_root, terms))
        .collect();
    let argv = provider.agent_facade_argv_from_provider_argv(argv);
    DecisionRoute {
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        binary: "asp".to_string(),
        kind,
        argv,
        stdin_mode: template.stdin_mode,
    }
}

fn expand_query_arg(
    arg: &str,
    selector: &str,
    project_root: &str,
    terms: &[String],
) -> Vec<String> {
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
            .replace("{projectRoot}", project_root),
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
