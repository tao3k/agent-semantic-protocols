//! Thin CLI adapter for the Rust-owned Gerbil deps index.

use agent_semantic_client_db::{
    DEFAULT_GERBIL_DEPS_SEARCH_LIMIT, GerbilDepsQueryRequest, GerbilDepsSearchRequest,
    gerbil_deps_minimal_import, gerbil_deps_query_export, gerbil_deps_query_terms,
    gerbil_deps_search_exports, gerbil_deps_selector_for, gerbil_deps_validate_module_id,
    gerbil_deps_validate_symbol,
};

const LANGUAGE_ID: &str = "gerbil-scheme";
const NAMESPACE: &str = "gerbil";

pub(super) fn try_run_gerbil_deps_index_command(
    language_id: &str,
    args: &[String],
) -> Result<bool, String> {
    if language_id != LANGUAGE_ID {
        return Ok(false);
    }
    if let Some(request) = parse_search_request(args)? {
        let result = gerbil_deps_search_exports(&request)?;
        println!(
            "[gerbil-deps] namespace=gerbil authority=active-gxi module={} scope={}",
            result.module_id, result.scope
        );
        if result.exports.is_empty() {
            println!(
                "|missing kind=export query=\"{}\"",
                escape_field(&result.query)
            );
            println!(
                "|next command=\"asp gerbil-scheme search deps gerbil {} items --query <more-specific-symbol>\"",
                result.module_id
            );
            return Ok(true);
        }
        println!(
            "|use import=\"{}\"",
            escape_field(&gerbil_deps_minimal_import(
                &result.module_id,
                &result.exports
            ))
        );
        for name in result.exports {
            println!(
                "|item kind=export name={} selector={}",
                name,
                gerbil_deps_selector_for(&result.module_id, &name)
            );
        }
        return Ok(true);
    }
    if let Some(request) = parse_query_request(args)? {
        let result = gerbil_deps_query_export(&request)?;
        println!(";;; selector: {}", result.selector);
        println!(
            ";;; import: {}",
            gerbil_deps_minimal_import(
                &result.module_id,
                std::slice::from_ref(&result.export_name)
            )
        );
        if let Some(line) = result.source_line {
            println!(";;; source: {}:{line}", result.source_path.display());
        } else {
            println!(";;; source: {}", result.source_path.display());
        }
        print!("{}", result.source_text);
        if !result.source_text.ends_with('\n') {
            println!();
        }
        return Ok(true);
    }
    Ok(false)
}

fn parse_search_request(args: &[String]) -> Result<Option<GerbilDepsSearchRequest>, String> {
    if !matches!(args.first().map(String::as_str), Some("search")) {
        return Ok(None);
    }
    if !matches!(args.get(1).map(String::as_str), Some("deps" | "dependency")) {
        return Ok(None);
    }
    if !matches!(args.get(2).map(String::as_str), Some(NAMESPACE)) {
        return Ok(None);
    }
    let Some(module_id) = args.get(3) else {
        return Err(search_usage_error("missing-module"));
    };
    gerbil_deps_validate_module_id(module_id).map_err(|reason| search_usage_error(&reason))?;
    if !matches!(args.get(4).map(String::as_str), Some("items")) {
        return Err(search_usage_error("items-required"));
    }

    let mut query = None;
    let mut limit = DEFAULT_GERBIL_DEPS_SEARCH_LIMIT;
    let mut index = 5;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--query" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(search_usage_error("query-value-required"));
                };
                query = Some(value.clone());
                index += 2;
            }
            "--limit" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(search_usage_error("limit-value-required"));
                };
                limit = parse_limit(value)?;
                index += 2;
            }
            "--workspace" | "--view" => {
                index += if args.get(index + 1).is_some() { 2 } else { 1 };
            }
            _ if arg.starts_with("--query=") => {
                query = Some(arg["--query=".len()..].to_string());
                index += 1;
            }
            _ if arg.starts_with("--limit=") => {
                limit = parse_limit(&arg["--limit=".len()..])?;
                index += 1;
            }
            _ if arg.starts_with("--workspace=") || arg.starts_with("--view=") => {
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    let query = query
        .map(|query| query.trim().to_string())
        .filter(|query| !query.is_empty())
        .ok_or_else(|| search_usage_error("query-required"))?;
    let terms = gerbil_deps_query_terms(&query);
    if terms.is_empty() {
        return Err(search_usage_error("query-required"));
    }
    Ok(Some(GerbilDepsSearchRequest {
        module_id: module_id.clone(),
        query,
        terms,
        limit,
    }))
}

fn parse_query_request(args: &[String]) -> Result<Option<GerbilDepsQueryRequest>, String> {
    if !matches!(args.first().map(String::as_str), Some("query")) {
        return Ok(None);
    }
    let Some(selector) = gerbil_selector_arg(args) else {
        return Ok(None);
    };
    let Some(rest) = selector.strip_prefix("gerbil:/") else {
        return Ok(None);
    };
    let Some((module_path, export_name)) = rest.split_once("#export/") else {
        return Err(format!(
            "unsupported Gerbil selector `{selector}`; use gerbil:/std/srfi/13#export/string-prefix?"
        ));
    };
    if module_path.is_empty() || export_name.is_empty() {
        return Err(format!(
            "invalid Gerbil selector `{selector}`; use gerbil:/std/srfi/13#export/string-prefix?"
        ));
    }
    let module_id = format!(":{module_path}");
    gerbil_deps_validate_module_id(&module_id).map_err(|reason| search_usage_error(&reason))?;
    gerbil_deps_validate_symbol(export_name)?;
    Ok(Some(GerbilDepsQueryRequest {
        selector: selector.clone(),
        module_id,
        export_name: export_name.to_string(),
    }))
}

fn gerbil_selector_arg(args: &[String]) -> Option<String> {
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--selector" {
            return args.get(index + 1).cloned();
        }
        if let Some(selector) = arg.strip_prefix("--selector=") {
            return Some(selector.to_string());
        }
        if arg.starts_with("gerbil:/") {
            return Some(arg.clone());
        }
        index += 1;
    }
    None
}

fn parse_limit(value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map(|limit| limit.clamp(1, DEFAULT_GERBIL_DEPS_SEARCH_LIMIT))
        .map_err(|_| search_usage_error("limit-must-be-integer"))
}

fn search_usage_error(reason: &str) -> String {
    format!(
        "[gerbil-deps] namespace=gerbil status=blocked reason={reason}\n\
deps gerbil requires a specific module and item query; use: asp gerbil-scheme search deps gerbil :std/srfi/13 items --query string-prefix"
    )
}

fn escape_field(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
