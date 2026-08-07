//! Facade argument extraction for provider-native projections.

use std::path::Path;

pub(super) fn provider_native_argument_values(
    method: &str,
    args: &[String],
    resolved_workspace: &Path,
) -> Result<agent_semantic_hook::ProviderMethodArgumentValuesV1, String> {
    let unavailable = |detail: &str| {
        format!(
            "reasonKind=provider-native-argument-projection-unavailable method={method} detail={detail}"
        )
    };
    let expected = method
        .split_once('/')
        .ok_or_else(|| unavailable("invalid-method"))?;
    if args.first().map(String::as_str) != Some(expected.0)
        || args.get(1).map(String::as_str) != Some(expected.1)
    {
        return Err(unavailable("facade-method-mismatch"));
    }

    let workspace = resolved_workspace
        .to_str()
        .ok_or_else(|| unavailable("workspace-not-utf8"))?;
    let mut values = agent_semantic_hook::ProviderMethodArgumentValuesV1 {
        workspace: Some(workspace.to_string()),
        ..Default::default()
    };
    let mut queries = Vec::new();
    let mut positionals = Vec::new();
    let mut index = 2usize;
    while index < args.len() {
        let argument = &args[index];
        match argument.as_str() {
            "--query" | "--query-set" => {
                queries.push(required_value(args, index, "query", &unavailable)?.clone());
                index += 2;
            }
            "--view" => {
                values.presentation =
                    Some(required_value(args, index, "presentation", &unavailable)?.clone());
                index += 2;
            }
            "--owner" => {
                values.owner = Some(required_value(args, index, "owner", &unavailable)?.clone());
                index += 2;
            }
            value if value.starts_with("--query=") || value.starts_with("--query-set=") => {
                queries.push(equals_value(value));
                index += 1;
            }
            value if value.starts_with("--view=") => {
                values.presentation = Some(equals_value(value));
                index += 1;
            }
            value if value.starts_with("--owner=") => {
                values.owner = Some(equals_value(value));
                index += 1;
            }
            value if value.starts_with('-') => {
                return Err(unavailable("unsupported-facade-option"));
            }
            value => {
                positionals.push(value.to_string());
                index += 1;
            }
        }
    }

    if method == "search/lexical" {
        if queries.is_empty() {
            queries.extend(
                positionals
                    .iter()
                    .filter(|value| !matches!(value.as_str(), "owner" | "items" | "tests"))
                    .cloned(),
            );
        }
        values.query = (!queries.is_empty()).then(|| queries.join(" "));
        values.owner = None;
    } else if method.starts_with("search/owner") {
        values.owner = values.owner.or_else(|| positionals.first().cloned());
        values.query = (!queries.is_empty()).then(|| queries.join(" "));
    } else {
        return Err(unavailable("unsupported-method"));
    }
    Ok(values)
}

fn required_value<'a>(
    args: &'a [String],
    index: usize,
    field: &str,
    unavailable: &impl Fn(&str) -> String,
) -> Result<&'a String, String> {
    args.get(index + 1)
        .ok_or_else(|| unavailable(&format!("missing-{field}")))
}

fn equals_value(argument: &str) -> String {
    argument
        .split_once('=')
        .expect("caller matched equals form")
        .1
        .to_string()
}

#[cfg(test)]
#[path = "../../tests/unit/command/provider_argument_projection.rs"]
mod tests;
