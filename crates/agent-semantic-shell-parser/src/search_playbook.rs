// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Boundary-only parsing for one `asp search playbook` argv.
//!
//! This parser retains every native token byte-for-byte after Host shell
//! tokenization. It uses the native rg option-arity table only to distinguish
//! an rg option value from a top-level PlayBook boundary; semantic admission
//! remains native-parser owned.

const PLAYBOOK_OPTIONS: &[&str] = &[
    "--language",
    "--documents",
    "--workspace",
    "--rg",
    "--tantivy",
    "--syntax",
    "--native-syntax",
    "--graph",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPlaybookGlobalKind {
    Language,
    Documents,
    Workspace,
}

impl SearchPlaybookGlobalKind {
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::Language => "language",
            Self::Documents => "documents",
            Self::Workspace => "workspace",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPlaybookBlockKind {
    Rg,
    Tantivy,
    Syntax,
    NativeSyntax,
    Graph,
}

impl SearchPlaybookBlockKind {
    pub const fn option(self) -> &'static str {
        match self {
            Self::Rg => "--rg",
            Self::Tantivy => "--tantivy",
            Self::Syntax => "--syntax",
            Self::NativeSyntax => "--native-syntax",
            Self::Graph => "--graph",
        }
    }

    pub const fn field_name(self) -> &'static str {
        match self {
            Self::Rg => "rg",
            Self::Tantivy => "tantivy",
            Self::Syntax => "syntax",
            Self::NativeSyntax => "nativeSyntax",
            Self::Graph => "graph",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPlaybookGlobal {
    pub kind: SearchPlaybookGlobalKind,
    pub option_token_index: usize,
    pub value_token_index: usize,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPlaybookNativeBlock {
    pub kind: SearchPlaybookBlockKind,
    pub block_index: usize,
    pub option_token_index: usize,
    pub argv_start_token_index: usize,
    pub argv_end_token_index_exclusive: usize,
    pub argv: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPlaybookIssueKind {
    InvalidOperation,
    MissingValue,
    DuplicateGlobal,
    UnknownOption,
    InvalidProducerExpression,
    InvalidWorkspaceIdentity,
    InvalidSyntaxProducer,
    InvalidNativeSelector,
    InvalidGraphLanguage,
    PairedInputRequired,
    GraphBeforeInput,
    InputAfterGraph,
    IncompleteSyntax,
    IncompleteGraph,
    NativeSyntaxArity,
    RgSyntaxInvalid,
    RgRootOutsideGeneration,
    RgSecondaryProcessForbidden,
    RgOperationNotSearch,
    RgOutputNotAttributable,
    TantivySyntaxInvalid,
    TantivyFieldUnsupported,
    TantivyExpressionTooSimple,
}

impl SearchPlaybookIssueKind {
    pub const fn reason_kind(self) -> &'static str {
        match self {
            Self::InvalidOperation => "search-playbook-operation-invalid",
            Self::MissingValue => "search-playbook-field-missing-value",
            Self::DuplicateGlobal => "search-playbook-field-conflict",
            Self::UnknownOption => "search-playbook-option-unsupported",
            Self::InvalidProducerExpression => "search-playbook-producer-expression-invalid",
            Self::InvalidWorkspaceIdentity => "search-playbook-workspace-identity-invalid",
            Self::InvalidSyntaxProducer => "search-playbook-syntax-producer-invalid",
            Self::InvalidNativeSelector => "search-playbook-native-selector-invalid",
            Self::InvalidGraphLanguage => "search-playbook-graph-language-invalid",
            Self::PairedInputRequired => "search-playbook-request-incomplete",
            Self::GraphBeforeInput | Self::InputAfterGraph => {
                "search-playbook-layout-order-invalid"
            }
            Self::IncompleteSyntax | Self::IncompleteGraph | Self::NativeSyntaxArity => {
                "search-playbook-native-block-incomplete"
            }
            Self::RgSyntaxInvalid => "search-playbook-rg-syntax-invalid",
            Self::RgRootOutsideGeneration => "search-playbook-rg-root-outside-generation",
            Self::RgSecondaryProcessForbidden => "search-playbook-rg-secondary-process-forbidden",
            Self::RgOperationNotSearch => "search-playbook-rg-operation-not-search",
            Self::RgOutputNotAttributable => "search-playbook-rg-output-not-attributable",
            Self::TantivySyntaxInvalid => "search-playbook-tantivy-syntax-invalid",
            Self::TantivyFieldUnsupported => "search-playbook-tantivy-field-unsupported",
            Self::TantivyExpressionTooSimple => "search-playbook-tantivy-expression-too-simple",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPlaybookIssue {
    pub kind: SearchPlaybookIssueKind,
    pub field: &'static str,
    pub token_index: Option<usize>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPlaybookBoundaryParse {
    pub globals: Vec<SearchPlaybookGlobal>,
    pub blocks: Vec<SearchPlaybookNativeBlock>,
    pub issues: Vec<SearchPlaybookIssue>,
    pub missing_fields: Vec<&'static str>,
    pub conflicting_fields: Vec<&'static str>,
    pub rg_analyses: Vec<crate::NativeRgArgvAnalysis>,
    pub tantivy_analyses: Vec<crate::TantivyQueryAnalysis>,
}

impl SearchPlaybookBoundaryParse {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn global_value(&self, kind: SearchPlaybookGlobalKind) -> Option<&str> {
        self.globals
            .iter()
            .find(|global| global.kind == kind)
            .map(|global| global.value.as_str())
    }
}

pub fn parse_search_playbook_boundaries(args: &[String]) -> SearchPlaybookBoundaryParse {
    let mut parsed = SearchPlaybookBoundaryParse {
        globals: Vec::new(),
        blocks: Vec::new(),
        issues: Vec::new(),
        missing_fields: Vec::new(),
        conflicting_fields: Vec::new(),
        rg_analyses: Vec::new(),
        tantivy_analyses: Vec::new(),
    };
    if args.first().map(String::as_str) != Some("search")
        || args.get(1).map(String::as_str) != Some("playbook")
    {
        parsed.issues.push(issue(
            SearchPlaybookIssueKind::InvalidOperation,
            "operation",
            Some(0),
            "use `asp search playbook`",
        ));
        return parsed;
    }

    let mut index = 2;
    let mut graph_started = false;
    while index < args.len() {
        let option_index = index;
        match args[index].as_str() {
            "--language" | "--documents" | "--workspace" => {
                let kind = match args[index].as_str() {
                    "--language" => SearchPlaybookGlobalKind::Language,
                    "--documents" => SearchPlaybookGlobalKind::Documents,
                    _ => SearchPlaybookGlobalKind::Workspace,
                };
                let field = kind.field_name();
                let Some(value) = args.get(index + 1).filter(|value| !is_boundary(value)) else {
                    parsed.missing_fields.push(field);
                    parsed.issues.push(issue(
                        SearchPlaybookIssueKind::MissingValue,
                        field,
                        Some(option_index),
                        format!("{} requires exactly one value", args[index]),
                    ));
                    index += 1;
                    continue;
                };
                if parsed.globals.iter().any(|global| global.kind == kind) {
                    parsed.conflicting_fields.push(field);
                    parsed.issues.push(issue(
                        SearchPlaybookIssueKind::DuplicateGlobal,
                        field,
                        Some(option_index),
                        format!("{} may occur only once", args[index]),
                    ));
                }
                if matches!(
                    kind,
                    SearchPlaybookGlobalKind::Language | SearchPlaybookGlobalKind::Documents
                ) && !valid_producer_expression(value)
                {
                    parsed.issues.push(issue(
                        SearchPlaybookIssueKind::InvalidProducerExpression,
                        field,
                        Some(index + 1),
                        format!("{} is not a registered-producer expression", args[index]),
                    ));
                }
                if kind == SearchPlaybookGlobalKind::Workspace && !valid_registered_name(value) {
                    parsed.issues.push(issue(
                        SearchPlaybookIssueKind::InvalidWorkspaceIdentity,
                        field,
                        Some(index + 1),
                        "--workspace requires one registered WorkspaceId, not a filesystem path",
                    ));
                }
                parsed.globals.push(SearchPlaybookGlobal {
                    kind,
                    option_token_index: option_index,
                    value_token_index: index + 1,
                    value: value.clone(),
                });
                index += 2;
            }
            "--rg" | "--tantivy" | "--syntax" | "--native-syntax" | "--graph" => {
                let kind = match args[index].as_str() {
                    "--rg" => SearchPlaybookBlockKind::Rg,
                    "--tantivy" => SearchPlaybookBlockKind::Tantivy,
                    "--syntax" => SearchPlaybookBlockKind::Syntax,
                    "--native-syntax" => SearchPlaybookBlockKind::NativeSyntax,
                    _ => SearchPlaybookBlockKind::Graph,
                };
                let start = index + 1;
                let end = if kind == SearchPlaybookBlockKind::Rg {
                    native_rg_block_end(args, start)
                } else {
                    native_block_end(args, start)
                };
                let block_index = parsed
                    .blocks
                    .iter()
                    .filter(|block| block.kind == kind)
                    .count();
                parsed.blocks.push(SearchPlaybookNativeBlock {
                    kind,
                    block_index,
                    option_token_index: option_index,
                    argv_start_token_index: start,
                    argv_end_token_index_exclusive: end,
                    argv: args[start..end].to_vec(),
                });
                validate_block(&mut parsed, kind, option_index, start, end, graph_started);
                if kind == SearchPlaybookBlockKind::Rg && start < end {
                    let analysis = crate::analyze_native_rg_argv(&args[start..end]);
                    for diagnostic in &analysis.diagnostics {
                        let kind = match diagnostic.kind {
                            crate::NativeRgDiagnosticKind::UnknownOption
                            | crate::NativeRgDiagnosticKind::MissingOptionValue
                            | crate::NativeRgDiagnosticKind::MissingPattern => {
                                SearchPlaybookIssueKind::RgSyntaxInvalid
                            }
                            crate::NativeRgDiagnosticKind::ImmutableRootViolation => {
                                SearchPlaybookIssueKind::RgRootOutsideGeneration
                            }
                            crate::NativeRgDiagnosticKind::SecondaryProcessForbidden => {
                                SearchPlaybookIssueKind::RgSecondaryProcessForbidden
                            }
                            crate::NativeRgDiagnosticKind::NonSearchOperation => {
                                SearchPlaybookIssueKind::RgOperationNotSearch
                            }
                            crate::NativeRgDiagnosticKind::OutputNotAttributable => {
                                SearchPlaybookIssueKind::RgOutputNotAttributable
                            }
                        };
                        parsed.issues.push(issue(
                            kind,
                            "rg",
                            diagnostic.argv_token_index.map(|token| start + token),
                            diagnostic.message.clone(),
                        ));
                    }
                    parsed.rg_analyses.push(analysis);
                }
                if kind == SearchPlaybookBlockKind::Tantivy && start < end {
                    let expression = args[start..end].join(" ");
                    let analysis = crate::analyze_tantivy_query(&expression);
                    if !analysis.syntax_diagnostics.is_empty() {
                        parsed.issues.push(issue(
                            SearchPlaybookIssueKind::TantivySyntaxInvalid,
                            "tantivy",
                            Some(start),
                            format!(
                                "native Tantivy query grammar rejected block {block_index}: {}",
                                analysis
                                    .syntax_diagnostics
                                    .iter()
                                    .map(|diagnostic| diagnostic.message.as_str())
                                    .collect::<Vec<_>>()
                                    .join("; ")
                            ),
                        ));
                    }
                    if !analysis.unsupported_fields.is_empty() {
                        parsed.issues.push(issue(
                            SearchPlaybookIssueKind::TantivyFieldUnsupported,
                            "tantivy",
                            Some(start),
                            format!(
                                "Tantivy fields are not present in the Search index: {}",
                                analysis.unsupported_fields.join(",")
                            ),
                        ));
                    }
                    if !analysis.missing_features.is_empty() {
                        parsed.issues.push(issue(
                            SearchPlaybookIssueKind::TantivyExpressionTooSimple,
                            "tantivy",
                            Some(start),
                            format!(
                                "Tantivy expression is missing required structure: {}",
                                analysis.missing_features.join(",")
                            ),
                        ));
                    }
                    parsed.tantivy_analyses.push(analysis);
                }
                if kind == SearchPlaybookBlockKind::Graph {
                    graph_started = true;
                }
                index = end.max(index + 1);
            }
            option => {
                parsed.issues.push(issue(
                    SearchPlaybookIssueKind::UnknownOption,
                    "option",
                    Some(option_index),
                    format!("search playbook does not support option `{option}`"),
                ));
                index += 1;
            }
        }
    }

    let has_rg = parsed
        .blocks
        .iter()
        .any(|block| block.kind == SearchPlaybookBlockKind::Rg);
    let has_tantivy = parsed
        .blocks
        .iter()
        .any(|block| block.kind == SearchPlaybookBlockKind::Tantivy);
    if parsed
        .global_value(SearchPlaybookGlobalKind::Language)
        .is_none()
        && parsed
            .global_value(SearchPlaybookGlobalKind::Documents)
            .is_none()
    {
        parsed.missing_fields.push("producer");
        parsed.issues.push(issue(
            SearchPlaybookIssueKind::MissingValue,
            "producer",
            None,
            "Search Playbook requires --language or --documents",
        ));
    }
    for (present, missing) in [(has_rg, "rg"), (has_tantivy, "tantivy")] {
        if present {
            continue;
        }
        parsed.missing_fields.push(missing);
        parsed.issues.push(issue(
            SearchPlaybookIssueKind::PairedInputRequired,
            missing,
            None,
            "the Search Layout requires --rg and --tantivy to establish file context",
        ));
    }
    parsed.missing_fields.sort_unstable();
    parsed.missing_fields.dedup();
    parsed.conflicting_fields.sort_unstable();
    parsed.conflicting_fields.dedup();
    parsed
}

fn validate_block(
    parsed: &mut SearchPlaybookBoundaryParse,
    kind: SearchPlaybookBlockKind,
    option_index: usize,
    start: usize,
    end: usize,
    graph_started: bool,
) {
    let len = end.saturating_sub(start);
    if len == 0 {
        parsed.missing_fields.push(kind.field_name());
        parsed.issues.push(issue(
            SearchPlaybookIssueKind::MissingValue,
            kind.field_name(),
            Some(option_index),
            format!("{} requires a native argv block", kind.option()),
        ));
    }
    if graph_started && kind != SearchPlaybookBlockKind::Graph {
        parsed.conflicting_fields.push(kind.field_name());
        parsed.issues.push(issue(
            SearchPlaybookIssueKind::InputAfterGraph,
            kind.field_name(),
            Some(option_index),
            "retrieval and syntax inputs must precede the Graph barrier",
        ));
    }
    let has_preceding_input = parsed.blocks[..parsed.blocks.len().saturating_sub(1)]
        .iter()
        .any(|block| block.kind != SearchPlaybookBlockKind::Graph);
    match kind {
        SearchPlaybookBlockKind::Syntax => {
            if len < 2 {
                parsed.issues.push(issue(
                    SearchPlaybookIssueKind::IncompleteSyntax,
                    "syntax",
                    Some(option_index),
                    "--syntax requires a registered producer and native query argv",
                ));
            } else if !valid_registered_name(&parsed.blocks.last().expect("current block").argv[0])
            {
                parsed.issues.push(issue(
                    SearchPlaybookIssueKind::InvalidSyntaxProducer,
                    "syntax",
                    Some(start),
                    "--syntax producer must be one registered producer name",
                ));
            }
        }
        SearchPlaybookBlockKind::NativeSyntax => {
            if len != 1 {
                parsed.issues.push(issue(
                    SearchPlaybookIssueKind::NativeSyntaxArity,
                    "nativeSyntax",
                    Some(option_index),
                    "--native-syntax requires exactly one canonical selector",
                ));
            } else if !valid_exact_selector(&parsed.blocks.last().expect("current block").argv[0]) {
                parsed.issues.push(issue(
                    SearchPlaybookIssueKind::InvalidNativeSelector,
                    "nativeSyntax",
                    Some(start),
                    "--native-syntax requires a canonical parser-owned exact selector",
                ));
            }
        }
        SearchPlaybookBlockKind::Graph if !has_preceding_input => parsed.issues.push(issue(
            SearchPlaybookIssueKind::GraphBeforeInput,
            "graph",
            Some(option_index),
            "--graph requires a preceding retrieval or syntax input",
        )),
        SearchPlaybookBlockKind::Graph => {
            if len < 2 {
                parsed.issues.push(issue(
                    SearchPlaybookIssueKind::IncompleteGraph,
                    "graph",
                    Some(option_index),
                    "--graph requires a registered graph language and native argv",
                ));
            } else if !matches!(
                parsed.blocks.last().expect("current block").argv[0].as_str(),
                "gql" | "pgql"
            ) {
                parsed.issues.push(issue(
                    SearchPlaybookIssueKind::InvalidGraphLanguage,
                    "graph",
                    Some(start),
                    "--graph language must be gql or pgql for the V1 adapter",
                ));
            }
        }
        _ => {}
    }
}

fn issue(
    kind: SearchPlaybookIssueKind,
    field: &'static str,
    token_index: Option<usize>,
    message: impl Into<String>,
) -> SearchPlaybookIssue {
    SearchPlaybookIssue {
        kind,
        field,
        token_index,
        message: message.into(),
    }
}

fn is_boundary(value: &str) -> bool {
    PLAYBOOK_OPTIONS.contains(&value)
}

fn native_block_end(args: &[String], start: usize) -> usize {
    args[start..]
        .iter()
        .position(|token| is_boundary(token))
        .map_or(args.len(), |offset| start + offset)
}

fn native_rg_block_end(args: &[String], start: usize) -> usize {
    let mut index = start;
    let mut options_done = false;
    let mut explicit_pattern = false;
    let mut positional_count = 0usize;
    while index < args.len() {
        let token = &args[index];
        if is_boundary(token) {
            // Native `--` makes the first positional token the pattern when no
            // explicit -e/-f pattern or --files mode was selected. Preserve
            // that token even when it is spelled like a PlayBook option. Once
            // the pattern exists, a literal path with a reserved spelling is
            // written as ./--option to keep the grammar unambiguous.
            if options_done && !explicit_pattern && positional_count == 0 {
                positional_count += 1;
                index += 1;
                continue;
            }
            break;
        }
        if !options_done && token == "--" {
            options_done = true;
            index += 1;
            continue;
        }
        if !options_done && token.starts_with('-') && token != "-" {
            explicit_pattern |= crate::native_rg::option_supplies_pattern(token);
            let consumes_following = crate::native_rg::option_consumes_following_value(token);
            index += 1;
            if consumes_following && index < args.len() {
                // The value belongs to rg even if it is `--graph`,
                // `--syntax`, or another top-level-looking token.
                index += 1;
            }
            continue;
        }
        positional_count += 1;
        index += 1;
    }
    index
}

fn valid_producer_expression(value: &str) -> bool {
    !value.is_empty() && value.split('|').all(valid_registered_name)
}

fn valid_registered_name(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
}

fn valid_exact_selector(value: &str) -> bool {
    !value.contains(char::is_whitespace)
        && value.split_once("://").is_some_and(|(producer, rest)| {
            valid_registered_name(producer)
                && rest
                    .split_once("#item/")
                    .is_some_and(|(owner, item)| !owner.is_empty() && !item.is_empty())
        })
}
