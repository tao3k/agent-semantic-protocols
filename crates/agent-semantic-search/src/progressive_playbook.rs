// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Public Search Playbook Scheme expression and normalized request lowering.

use agent_semantic_tree_sitter::SchemeDatum;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressiveSearchPlaybookRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rg: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tantivy: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub syntax: Vec<ProducerNativeBlock>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub native_syntax: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub graph: Vec<GraphNativeBlock>,
    pub clause_order: Vec<SearchPlaybookClauseRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProducerNativeBlock {
    pub producer: String,
    pub argv: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphNativeBlock {
    pub language: String,
    pub argv: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPlaybookProducerDeclaration {
    pub language: Vec<String>,
    pub documents: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchPlaybookClauseAxis {
    Rg,
    Tantivy,
    Syntax,
    #[serde(rename = "native-syntax")]
    NativeSyntax,
    Graph,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookClauseRef {
    pub axis: SearchPlaybookClauseAxis,
    pub block_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgressiveSearchPlaybookError {
    InvalidOperation,
    IncompleteRequest(String),
    InvalidClauseOrder(String),
    UnsupportedOption(String),
    InvalidParameter {
        reason_kind: &'static str,
        message: String,
    },
    InvalidRg {
        reason_kind: &'static str,
        message: String,
    },
    InvalidTantivy {
        reason_kind: &'static str,
        message: String,
    },
}

impl std::fmt::Display for ProgressiveSearchPlaybookError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOperation => formatter.write_str("use `asp search playbook`"),
            Self::IncompleteRequest(reason) => {
                write!(formatter, "search-playbook-request-incomplete: {reason}")
            }
            Self::InvalidClauseOrder(reason) => formatter.write_str(reason),
            Self::UnsupportedOption(operator) => {
                write!(
                    formatter,
                    "Search Playbook does not support operator `{operator}`"
                )
            }
            Self::InvalidParameter { message, .. }
            | Self::InvalidRg { message, .. }
            | Self::InvalidTantivy { message, .. } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ProgressiveSearchPlaybookError {}

impl ProgressiveSearchPlaybookError {
    pub fn reason_kind(&self) -> &'static str {
        match self {
            Self::IncompleteRequest(_) => "search-playbook-request-incomplete",
            Self::InvalidOperation => "search-playbook-operation-invalid",
            Self::InvalidClauseOrder(_) => "search-playbook-clause-order-invalid",
            Self::UnsupportedOption(_) => "search-playbook-operator-unsupported",
            Self::InvalidParameter { reason_kind, .. }
            | Self::InvalidRg { reason_kind, .. }
            | Self::InvalidTantivy { reason_kind, .. } => reason_kind,
        }
    }
}

pub fn parse_progressive_search_playbook_args(
    args: &[String],
) -> Result<ProgressiveSearchPlaybookRequest, ProgressiveSearchPlaybookError> {
    if args.first().map(String::as_str) != Some("search")
        || args.get(1).map(String::as_str) != Some("playbook")
    {
        return Err(ProgressiveSearchPlaybookError::InvalidOperation);
    }
    let Some(source) = args.get(2) else {
        return Err(incomplete("one Scheme composition expression is required"));
    };
    if args.len() != 3 {
        return Err(invalid_parameter(
            "search-playbook-source-arity-invalid",
            "Search Playbook accepts exactly one Scheme expression argument",
        ));
    }
    let datums = agent_semantic_tree_sitter::parse_scheme_datums(source).map_err(|error| {
        invalid_parameter(
            error.reason_kind,
            format!("invalid Search Playbook Scheme source: {error}"),
        )
    })?;
    let [SchemeDatum::List(root)] = datums.as_slice() else {
        return Err(invalid_parameter(
            "search-playbook-root-invalid",
            "expected one (search ...) expression",
        ));
    };
    lower_search(root)
}

/// Reads only the declared producer axes from one structurally valid Search
/// Playbook expression. Hook routing uses this before native leaf admission so
/// a malformed producer-owned request still reaches the typed PreTool denial.
pub fn parse_search_playbook_producer_declaration(
    source: &str,
) -> Result<SearchPlaybookProducerDeclaration, ProgressiveSearchPlaybookError> {
    let datums = agent_semantic_tree_sitter::parse_scheme_datums(source).map_err(|error| {
        invalid_parameter(
            error.reason_kind,
            format!("invalid Search Playbook Scheme source: {error}"),
        )
    })?;
    let [SchemeDatum::List(root)] = datums.as_slice() else {
        return Err(invalid_parameter(
            "search-playbook-root-invalid",
            "expected one (search ...) expression",
        ));
    };
    if symbol(root.first()) != Some("search") {
        return Err(invalid_parameter(
            "search-playbook-root-invalid",
            "expected one (search ...) expression",
        ));
    }
    let producers_index = match root.as_slice() {
        [_, SchemeDatum::List(target), _, _] if symbol(target.first()) == Some("workspace") => 2,
        [_, _, _] => 1,
        _ => {
            return Err(invalid_parameter(
                "search-playbook-root-invalid",
                "expected (search [(workspace \"id\")] (producers ...) composition)",
            ));
        }
    };
    let SchemeDatum::List(producers) = &root[producers_index] else {
        return Err(invalid_parameter(
            "search-playbook-producers-invalid",
            "expected a producers declaration",
        ));
    };
    let (language, documents) = lower_producers(producers)?;
    Ok(SearchPlaybookProducerDeclaration {
        language: split_producer_expression(language.as_deref()),
        documents: split_producer_expression(documents.as_deref()),
    })
}

fn split_producer_expression(expression: Option<&str>) -> Vec<String> {
    expression
        .into_iter()
        .flat_map(|expression| expression.split('|'))
        .map(str::to_owned)
        .collect()
}

fn lower_search(
    root: &[SchemeDatum],
) -> Result<ProgressiveSearchPlaybookRequest, ProgressiveSearchPlaybookError> {
    if symbol(root.first()) != Some("search") {
        return Err(invalid_parameter(
            "search-playbook-root-invalid",
            "expected one (search ...) expression",
        ));
    }
    let (workspace, producers_index, flow_index) = match root {
        [_, SchemeDatum::List(target), _, _] if symbol(target.first()) == Some("workspace") => {
            let [_, SchemeDatum::String(workspace)] = target.as_slice() else {
                return Err(invalid_parameter(
                    "search-playbook-workspace-identity-invalid",
                    "workspace requires exactly one registered WorkspaceId string",
                ));
            };
            if !valid_registered_name(workspace) {
                return Err(invalid_parameter(
                    "search-playbook-workspace-identity-invalid",
                    "workspace requires one registered WorkspaceId, not a path",
                ));
            }
            (Some(workspace.clone()), 2, 3)
        }
        [_, _, _] => (None, 1, 2),
        _ => {
            return Err(invalid_parameter(
                "search-playbook-root-invalid",
                "expected (search [(workspace \"id\")] (producers ...) composition)",
            ));
        }
    };
    let SchemeDatum::List(producers) = &root[producers_index] else {
        return Err(invalid_parameter(
            "search-playbook-producers-invalid",
            "expected a producers declaration",
        ));
    };
    let (language, documents) = lower_producers(producers)?;
    let mut state = LoweringState::default();
    lower_v1_composition(&root[flow_index], &mut state)?;
    if state.rg.is_empty() || state.tantivy.is_empty() {
        return Err(incomplete(
            "the default layout requires rg and tantivy leaves",
        ));
    }
    Ok(ProgressiveSearchPlaybookRequest {
        language,
        documents,
        workspace,
        rg: state.rg,
        tantivy: state.tantivy,
        syntax: state.syntax,
        native_syntax: state.native_syntax,
        graph: state.graph,
        clause_order: state.clause_order,
    })
}

fn lower_producers(
    form: &[SchemeDatum],
) -> Result<(Option<String>, Option<String>), ProgressiveSearchPlaybookError> {
    if symbol(form.first()) != Some("producers") {
        return Err(invalid_parameter(
            "search-playbook-producers-invalid",
            "expected (producers (language ...) (documents ...))",
        ));
    }
    let mut language = None;
    let mut documents = None;
    for axis in &form[1..] {
        let SchemeDatum::List(values) = axis else {
            return Err(invalid_parameter(
                "search-playbook-producers-invalid",
                "producer axis must be a list",
            ));
        };
        let Some(name) = symbol(values.first()) else {
            return Err(invalid_parameter(
                "search-playbook-producers-invalid",
                "producer axis must begin with language or documents",
            ));
        };
        let atoms = values[1..]
            .iter()
            .map(|value| match value {
                SchemeDatum::Symbol(value) if valid_registered_name(value) => Ok(value.as_str()),
                _ => Err(invalid_parameter(
                    "search-playbook-producer-expression-invalid",
                    "producer values must be registered-name symbols",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if atoms.is_empty() {
            return Err(incomplete(format!("{name} producer axis is empty")));
        }
        let joined = atoms.join("|");
        let target = match name {
            "language" => &mut language,
            "documents" => &mut documents,
            other => {
                return Err(ProgressiveSearchPlaybookError::UnsupportedOption(
                    other.to_owned(),
                ));
            }
        };
        if target.replace(joined).is_some() {
            return Err(invalid_parameter(
                "search-playbook-field-conflict",
                format!("producer axis `{name}` may occur only once"),
            ));
        }
    }
    if language.is_none() && documents.is_none() {
        return Err(incomplete("at least one producer axis is required"));
    }
    Ok((language, documents))
}

#[derive(Default)]
struct LoweringState {
    rg: Vec<Vec<String>>,
    tantivy: Vec<Vec<String>>,
    syntax: Vec<ProducerNativeBlock>,
    native_syntax: Vec<String>,
    graph: Vec<GraphNativeBlock>,
    clause_order: Vec<SearchPlaybookClauseRef>,
    graph_started: bool,
}

fn lower_v1_composition(
    datum: &SchemeDatum,
    state: &mut LoweringState,
) -> Result<(), ProgressiveSearchPlaybookError> {
    let form = operator_form(datum)?;
    match symbol(form.first()).expect("operator_form checks the head") {
        "chain" => {
            if form.len() < 2 {
                return Err(incomplete("chain requires a child"));
            }
            lower_acquisition(&form[1], state)?;
            for child in &form[2..] {
                lower_leaf(child, state)?;
            }
        }
        "intersect" => {
            lower_acquisition(datum, state)?;
        }
        "union" => {
            return Err(ProgressiveSearchPlaybookError::UnsupportedOption(
                "union".to_owned(),
            ));
        }
        other => {
            return Err(invalid_parameter(
                "search-playbook-layout-invalid",
                format!("V1 composition must begin with intersect(rg, tantivy), found `{other}`"),
            ));
        }
    }
    Ok(())
}

fn lower_acquisition(
    datum: &SchemeDatum,
    state: &mut LoweringState,
) -> Result<(), ProgressiveSearchPlaybookError> {
    let form = operator_form(datum)?;
    if symbol(form.first()) != Some("intersect") {
        return Err(invalid_parameter(
            "search-playbook-layout-invalid",
            "V1 begins with an intersect containing rg and tantivy leaves",
        ));
    }
    if form.len() < 3 {
        return Err(incomplete(
            "the acquisition intersect requires rg and tantivy leaves",
        ));
    }
    for child in &form[1..] {
        let child_form = operator_form(child)?;
        match symbol(child_form.first()).expect("operator_form checks the head") {
            "rg" | "tantivy" => lower_leaf(child, state)?,
            operator => {
                return Err(invalid_parameter(
                    "search-playbook-layout-invalid",
                    format!("V1 acquisition intersect does not admit `{operator}`"),
                ));
            }
        }
    }
    Ok(())
}

fn lower_leaf(
    datum: &SchemeDatum,
    state: &mut LoweringState,
) -> Result<(), ProgressiveSearchPlaybookError> {
    let form = operator_form(datum)?;
    let operator = symbol(form.first()).expect("operator_form checks the head");
    match operator {
        "rg" => {
            reject_after_graph(state)?;
            let argv = string_arguments(operator, &form[1..])?;
            require_nonempty(operator, &argv)?;
            validate_rg(&argv)?;
            let block_index = state.rg.len();
            state.rg.push(argv);
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Rg,
                block_index,
            });
        }
        "tantivy" => {
            reject_after_graph(state)?;
            let argv = string_arguments(operator, &form[1..])?;
            if argv.len() != 1 {
                return Err(incomplete("tantivy requires one query string"));
            }
            validate_tantivy(&argv[0])?;
            let block_index = state.tantivy.len();
            state.tantivy.push(argv);
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Tantivy,
                block_index,
            });
        }
        "syntax" => {
            reject_after_graph(state)?;
            let (producer, argv) = producer_arguments(operator, &form[1..])?;
            if argv.len() != 1 {
                return Err(incomplete(
                    "syntax requires exactly one enhanced Tree-sitter Query string",
                ));
            }
            let block_index = state.syntax.len();
            state.syntax.push(ProducerNativeBlock { producer, argv });
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Syntax,
                block_index,
            });
        }
        "native-syntax" => {
            reject_after_graph(state)?;
            let argv = string_arguments(operator, &form[1..])?;
            if argv.len() != 1 || !valid_exact_selector(&argv[0]) {
                return Err(invalid_parameter(
                    "search-playbook-native-selector-invalid",
                    "native-syntax requires one canonical parser-owned selector",
                ));
            }
            let block_index = state.native_syntax.len();
            state.native_syntax.push(argv[0].clone());
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::NativeSyntax,
                block_index,
            });
        }
        "graph" => {
            if state.clause_order.is_empty() {
                return Err(ProgressiveSearchPlaybookError::InvalidClauseOrder(
                    "graph requires a preceding retrieval or syntax leaf".to_owned(),
                ));
            }
            let (language, argv) = producer_arguments(operator, &form[1..])?;
            if !matches!(language.as_str(), "gql" | "pgql") {
                return Err(invalid_parameter(
                    "search-playbook-graph-language-invalid",
                    "graph language must be gql or pgql",
                ));
            }
            state.graph_started = true;
            let block_index = state.graph.len();
            state.graph.push(GraphNativeBlock { language, argv });
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Graph,
                block_index,
            });
        }
        "chain" | "intersect" | "union" => {
            return Err(invalid_parameter(
                "search-playbook-layout-invalid",
                format!("nested `{operator}` is not projected by the V1 normalized request"),
            ));
        }
        other => {
            return Err(ProgressiveSearchPlaybookError::UnsupportedOption(
                other.to_owned(),
            ));
        }
    }
    Ok(())
}

fn operator_form(datum: &SchemeDatum) -> Result<&[SchemeDatum], ProgressiveSearchPlaybookError> {
    let SchemeDatum::List(form) = datum else {
        return Err(invalid_parameter(
            "search-playbook-composition-invalid",
            "composition must be an operator list",
        ));
    };
    if symbol(form.first()).is_none() {
        return Err(invalid_parameter(
            "search-playbook-composition-invalid",
            "composition must begin with an operator",
        ));
    }
    Ok(form)
}

fn symbol(value: Option<&SchemeDatum>) -> Option<&str> {
    match value {
        Some(SchemeDatum::Symbol(value)) => Some(value),
        _ => None,
    }
}

fn string_arguments(
    operator: &str,
    values: &[SchemeDatum],
) -> Result<Vec<String>, ProgressiveSearchPlaybookError> {
    values
        .iter()
        .map(|value| match value {
            SchemeDatum::String(value) => Ok(value.clone()),
            _ => Err(invalid_parameter(
                "search-playbook-argument-invalid",
                format!("{operator} arguments must be strings"),
            )),
        })
        .collect()
}

fn producer_arguments(
    operator: &str,
    values: &[SchemeDatum],
) -> Result<(String, Vec<String>), ProgressiveSearchPlaybookError> {
    let Some(SchemeDatum::Symbol(producer)) = values.first() else {
        return Err(invalid_parameter(
            "search-playbook-producer-expression-invalid",
            format!("{operator} requires a producer symbol"),
        ));
    };
    if !valid_registered_name(producer) {
        return Err(invalid_parameter(
            "search-playbook-producer-expression-invalid",
            format!("invalid {operator} producer"),
        ));
    }
    let argv = string_arguments(operator, &values[1..])?;
    require_nonempty(operator, &argv)?;
    Ok((producer.clone(), argv))
}

fn reject_after_graph(state: &LoweringState) -> Result<(), ProgressiveSearchPlaybookError> {
    if state.graph_started {
        return Err(ProgressiveSearchPlaybookError::InvalidClauseOrder(
            "retrieval and syntax leaves must precede the Graph barrier".to_owned(),
        ));
    }
    Ok(())
}

fn incomplete(message: impl Into<String>) -> ProgressiveSearchPlaybookError {
    ProgressiveSearchPlaybookError::IncompleteRequest(message.into())
}

fn invalid_parameter(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> ProgressiveSearchPlaybookError {
    ProgressiveSearchPlaybookError::InvalidParameter {
        reason_kind,
        message: message.into(),
    }
}

fn require_nonempty(name: &str, values: &[String]) -> Result<(), ProgressiveSearchPlaybookError> {
    if values.is_empty() {
        return Err(incomplete(format!("{name} requires arguments")));
    }
    Ok(())
}

fn validate_rg(argv: &[String]) -> Result<(), ProgressiveSearchPlaybookError> {
    use agent_semantic_shell_parser::NativeRgDiagnosticKind;
    if let Some(diagnostic) = agent_semantic_shell_parser::analyze_native_rg_argv(argv)
        .diagnostics
        .first()
    {
        let reason_kind = match diagnostic.kind {
            NativeRgDiagnosticKind::UnknownOption
            | NativeRgDiagnosticKind::MissingOptionValue
            | NativeRgDiagnosticKind::MissingPattern => "search-playbook-rg-syntax-invalid",
            NativeRgDiagnosticKind::ImmutableRootViolation => {
                "search-playbook-rg-root-outside-generation"
            }
            NativeRgDiagnosticKind::SecondaryProcessForbidden => {
                "search-playbook-rg-secondary-process-forbidden"
            }
            NativeRgDiagnosticKind::NonSearchOperation => "search-playbook-rg-operation-not-search",
            NativeRgDiagnosticKind::OutputNotAttributable => {
                "search-playbook-rg-output-not-attributable"
            }
        };
        return Err(ProgressiveSearchPlaybookError::InvalidRg {
            reason_kind,
            message: diagnostic.message.clone(),
        });
    }
    Ok(())
}

fn validate_tantivy(expression: &str) -> Result<(), ProgressiveSearchPlaybookError> {
    let analysis = agent_semantic_shell_parser::analyze_tantivy_query(expression);
    let (reason_kind, message) = if !analysis.syntax_diagnostics.is_empty() {
        (
            "search-playbook-tantivy-syntax-invalid",
            analysis
                .syntax_diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        )
    } else if !analysis.unsupported_fields.is_empty() {
        (
            "search-playbook-tantivy-field-unsupported",
            format!(
                "unsupported Tantivy fields: {}",
                analysis.unsupported_fields.join(",")
            ),
        )
    } else if !analysis.missing_features.is_empty() {
        (
            "search-playbook-tantivy-expression-too-simple",
            format!(
                "Tantivy expression is missing: {}",
                analysis.missing_features.join(",")
            ),
        )
    } else {
        return Ok(());
    };
    Err(ProgressiveSearchPlaybookError::InvalidTantivy {
        reason_kind,
        message,
    })
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

#[cfg(test)]
#[path = "../tests/unit/progressive_playbook.rs"]
mod tests;
