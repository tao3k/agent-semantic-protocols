use std::path::PathBuf;

use super::structural_selector::parse_structural_owner_query;

pub(super) struct OwnerQueryRequest {
    pub(super) language_id: String,
    pub(super) owner_path: PathBuf,
    pub(super) names_only: bool,
    projection: OwnerQueryProjection,
}

pub(super) struct OwnerItemQuery {
    kind: Option<String>,
    term: String,
    code: bool,
    projection: &'static str,
}

pub(super) enum OwnerQueryProjection {
    Item(OwnerItemQuery),
}

impl OwnerQueryRequest {
    pub(super) fn item_query(&self) -> Option<&OwnerItemQuery> {
        match &self.projection {
            OwnerQueryProjection::Item(query) => Some(query),
        }
    }
}

impl OwnerItemQuery {
    pub(super) fn kind(&self) -> Option<&str> {
        self.kind.as_deref()
    }

    pub(super) fn term(&self) -> &str {
        &self.term
    }

    pub(super) fn projection(&self) -> &'static str {
        self.projection
    }

    pub(super) fn is_code_projection(&self) -> bool {
        self.code
    }
}

impl OwnerQueryRequest {
    pub(super) fn parse(language_id: &str, args: &[String]) -> Result<Option<Self>, String> {
        if !matches!(language_id, "rust" | "typescript" | "python" | "julia")
            || !matches!(args.first().map(String::as_str), Some("query"))
        {
            return Ok(None);
        }
        if has_any_arg(
            args,
            &[
                "--json",
                "--receipt-json",
                "--treesitter-query",
                "--catalog",
            ],
        ) {
            return Ok(None);
        }
        if let Some(request) = Self::parse_structural_selector(language_id, args)? {
            return Ok(Some(request));
        }
        if has_any_arg(args, &["--from-hook", "--selector"]) {
            return Ok(None);
        }
        let Some(term) = arg_value(args, "--term")
            .or_else(|| arg_value(args, "--query"))
            .map(ToString::to_string)
        else {
            return Ok(None);
        };
        let Some(owner_path) = first_positional_owner_arg(args) else {
            return Ok(None);
        };
        if owner_path == "." || owner_path.contains(':') {
            return Ok(None);
        }
        Ok(Some(Self {
            language_id: language_id.to_string(),
            owner_path: PathBuf::from(owner_path),
            names_only: args.iter().any(|arg| arg == "--names-only"),
            projection: OwnerQueryProjection::Item(OwnerItemQuery {
                kind: None,
                term,
                code: args.iter().any(|arg| arg == "--code"),
                projection: "outline",
            }),
        }))
    }

    fn parse_structural_selector(
        language_id: &str,
        args: &[String],
    ) -> Result<Option<Self>, String> {
        let Some(selector) = arg_value(args, "--selector") else {
            return Ok(None);
        };
        let from_hook = arg_value(args, "--from-hook").unwrap_or_else(|| {
            if args.iter().any(|arg| arg == "--code") {
                "item-skeleton"
            } else {
                "syntax-outline"
            }
        });
        let code = args.iter().any(|arg| arg == "--code");
        if from_hook == "direct-source-read" {
            return Ok(None);
        }
        if let Some(owner_path) = bare_file_selector(selector) {
            let term = arg_value(args, "--term")
                .or_else(|| arg_value(args, "--query"))
                .map(ToString::to_string);
            return match term {
                Some(term) => Ok(Some(Self {
                    language_id: language_id.to_string(),
                    owner_path,
                    names_only: args.iter().any(|arg| arg == "--names-only"),
                    projection: OwnerQueryProjection::Item(OwnerItemQuery {
                        kind: None,
                        term,
                        code,
                        projection: "outline",
                    }),
                })),
                None if code => {
                    let workspace = arg_value(args, "--workspace").unwrap_or(".");
                    Err(format!(
                        "invalid query --code selector `{selector}`: file selectors are not executable code selectors; query an exact parser-owned item selector such as {language_id}://path#item/function/name; recover with search owner <path> items\nselectorState=file-selector\nprojection=code\nallowed=false\nreason=file-selectors-are-not-code-selectors\nnextAction=materialize-owner-items\nnextCommand=asp {language_id} search owner {selector} items --workspace {workspace} --view seeds\nrequiredSelector={language_id}://{selector}#item/<kind>/<name>"
                    ))
                }
                None => Ok(None),
            };
        }
        let Some(structural) = parse_structural_owner_query(language_id, from_hook, selector)
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            language_id: language_id.to_string(),
            owner_path: structural.owner_path,
            names_only: args.iter().any(|arg| arg == "--names-only"),
            projection: OwnerQueryProjection::Item(OwnerItemQuery {
                kind: structural.kind,
                term: structural.term,
                code,
                projection: structural.projection,
            }),
        }))
    }
}

fn bare_file_selector(selector: &str) -> Option<PathBuf> {
    if selector.contains("://") || selector.contains('#') || selector.contains(':') {
        return None;
    }
    let path = PathBuf::from(selector);
    (path.extension().is_some() || path.components().count() > 1).then_some(path)
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
