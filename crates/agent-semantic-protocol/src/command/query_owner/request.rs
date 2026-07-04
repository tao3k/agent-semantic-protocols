use std::path::PathBuf;

use super::structural_selector::parse_structural_owner_query;

pub(super) struct OwnerQueryRequest {
    pub(super) language_id: String,
    pub(super) owner_path: PathBuf,
    pub(super) kind: Option<String>,
    pub(super) term: String,
    pub(super) names_only: bool,
    pub(super) code: bool,
    pub(super) projection: &'static str,
}

impl OwnerQueryRequest {
    pub(super) fn parse(language_id: &str, args: &[String]) -> Option<Self> {
        if !matches!(language_id, "rust" | "typescript" | "python" | "julia")
            || !matches!(args.first().map(String::as_str), Some("query"))
        {
            return None;
        }
        if let Some(request) = Self::parse_structural_selector(language_id, args) {
            return Some(request);
        }
        if has_any_arg(
            args,
            &[
                "--json",
                "--receipt-json",
                "--treesitter-query",
                "--catalog",
                "--from-hook",
                "--selector",
            ],
        ) {
            return None;
        }
        let term = arg_value(args, "--term")
            .or_else(|| arg_value(args, "--query"))
            .map(ToString::to_string)?;
        let owner_path = first_positional_owner_arg(args)?;
        if owner_path == "." || owner_path.contains(':') {
            return None;
        }
        Some(Self {
            language_id: language_id.to_string(),
            owner_path: PathBuf::from(owner_path),
            kind: None,
            term,
            names_only: args.iter().any(|arg| arg == "--names-only"),
            code: args.iter().any(|arg| arg == "--code"),
            projection: "outline",
        })
    }

    fn parse_structural_selector(language_id: &str, args: &[String]) -> Option<Self> {
        let selector = arg_value(args, "--selector")?;
        let from_hook = arg_value(args, "--from-hook").unwrap_or_else(|| {
            if args.iter().any(|arg| arg == "--code") {
                "query-code"
            } else {
                "syntax-outline"
            }
        });
        let structural = parse_structural_owner_query(language_id, from_hook, selector)?;
        Some(Self {
            language_id: language_id.to_string(),
            owner_path: structural.owner_path,
            kind: structural.kind,
            term: structural.term,
            names_only: args.iter().any(|arg| arg == "--names-only"),
            code: args.iter().any(|arg| arg == "--code"),
            projection: structural.projection,
        })
    }
}

fn first_positional_owner_arg(args: &[String]) -> Option<&str> {
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        if arg.starts_with("--") {
            index += if option_takes_value(arg) { 2 } else { 1 };
            continue;
        }
        if arg.starts_with('-') || arg == "." {
            index += 1;
            continue;
        }
        return Some(arg);
    }
    None
}

fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let prefix = format!("{flag}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix))
        .or_else(|| {
            args.windows(2)
                .find_map(|window| (window[0] == flag).then_some(window[1].as_str()))
        })
}

fn has_any_arg(args: &[String], flags: &[&str]) -> bool {
    args.iter().any(|arg| {
        flags
            .iter()
            .any(|flag| arg == flag || arg.starts_with(&format!("{flag}=")))
    })
}

fn option_takes_value(arg: &str) -> bool {
    if arg.contains('=') {
        return false;
    }
    matches!(
        arg,
        "--term"
            | "--query"
            | "--workspace"
            | "--source"
            | "--from-hook"
            | "--selector"
            | "--treesitter-query"
            | "--catalog"
            | "--view"
    )
}
