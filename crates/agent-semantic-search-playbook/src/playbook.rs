// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Lightweight public Search Playbook V1 syntax and normalized request lowering.

use crate::model::{
    GraphNativeBlock, ProducerNativeBlock, ProgressiveSearchPlaybookError,
    ProgressiveSearchPlaybookRequest, SEARCH_PLAYBOOK_MAX_COMPOSITION_DEPTH,
    SEARCH_PLAYBOOK_MAX_COMPOSITION_NODES, SEARCH_PLAYBOOK_MAX_STATIC_WORK,
    SearchPlaybookClauseAxis, SearchPlaybookClauseRef, SearchPlaybookComposition,
    SearchPlaybookLeaf, SearchPlaybookNormalizedComposition, SearchPlaybookProducerDeclaration,
    TopologyOwnerMembershipBlock,
};
use agent_semantic_scheme_syntax::SchemeDatum;

/// Admits exactly `search playbook <scheme-expression>` and lowers its V1 AST.
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
    let datums = agent_semantic_scheme_syntax::parse_scheme_datums(source).map_err(|error| {
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
    let datums = agent_semantic_scheme_syntax::parse_scheme_datums(source).map_err(|error| {
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

/// Reads producer axes from the sole public Query Playbook Scheme expression.
/// Hook routing uses this lightweight structural view; the Client performs the
/// complete selector/projection lowering before Runtime dispatch.
pub fn parse_query_playbook_producer_declaration(
    source: &str,
) -> Result<SearchPlaybookProducerDeclaration, ProgressiveSearchPlaybookError> {
    let datums = agent_semantic_scheme_syntax::parse_scheme_datums(source).map_err(|error| {
        invalid_parameter(
            error.reason_kind,
            format!("invalid Query Playbook Scheme source: {error}"),
        )
    })?;
    let [SchemeDatum::List(root)] = datums.as_slice() else {
        return Err(invalid_parameter(
            "query-playbook-root-invalid",
            "expected one (query ...) expression",
        ));
    };
    if symbol(root.first()) != Some("query") {
        return Err(invalid_parameter(
            "query-playbook-root-invalid",
            "expected one (query ...) expression",
        ));
    }
    let producers_index = match root.as_slice() {
        [_, SchemeDatum::List(target), _, _] if symbol(target.first()) == Some("workspace") => 2,
        [_, _, _] => 1,
        _ => {
            return Err(invalid_parameter(
                "query-playbook-root-invalid",
                "expected (query [(workspace \"id\")] (producers ...) (select ...))",
            ));
        }
    };
    let SchemeDatum::List(producers) = &root[producers_index] else {
        return Err(invalid_parameter(
            "query-playbook-producers-invalid",
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
    let mut parsed_nodes = 0;
    let composition = parse_composition(&root[flow_index], 1, &mut parsed_nodes)?;
    admit_composition_budget(&composition)?;
    reject_duplicate_leaves(&composition)?;
    let mut state = LoweringState::default();
    lower_v1_composition(&composition, &mut state)?;
    if state.rg.is_empty()
        && state.tantivy.is_empty()
        && state.topology.is_empty()
        && state.syntax.is_empty()
        && state.native_syntax.is_empty()
    {
        return Err(incomplete(
            "Search Playbook requires a regex, ranked-text, topology, or structural acquisition leaf",
        ));
    }
    let mut normalized_indices = NormalizedLeafIndices::default();
    let normalized_composition =
        normalize_composition_references(&composition, &mut normalized_indices);
    Ok(ProgressiveSearchPlaybookRequest {
        language,
        documents,
        workspace,
        rg: state.rg,
        tantivy: state.tantivy,
        topology: state.topology,
        syntax: state.syntax,
        native_syntax: state.native_syntax,
        graph: state.graph,
        clause_order: state.clause_order,
        composition,
        normalized_composition,
    })
}

fn reject_duplicate_leaves(
    composition: &SearchPlaybookComposition,
) -> Result<(), ProgressiveSearchPlaybookError> {
    fn visit<'a>(
        composition: &'a SearchPlaybookComposition,
        seen: &mut Vec<&'a SearchPlaybookLeaf>,
    ) -> Result<(), ProgressiveSearchPlaybookError> {
        match composition {
            SearchPlaybookComposition::Chain(children)
            | SearchPlaybookComposition::Intersect(children) => {
                for child in children {
                    visit(child, seen)?;
                }
            }
            SearchPlaybookComposition::Leaf(leaf) => {
                if seen.contains(&leaf) {
                    return Err(invalid_parameter(
                        "search-playbook-redundant-predicate",
                        "Search Playbook rejects byte-identical predicate leaves as redundant work",
                    ));
                }
                seen.push(leaf);
            }
        }
        Ok(())
    }

    visit(composition, &mut Vec::new())
}

#[derive(Default)]
struct NormalizedLeafIndices {
    rg: usize,
    tantivy: usize,
    topology: usize,
    syntax: usize,
    native_syntax: usize,
    graph: usize,
}

fn normalize_composition_references(
    composition: &SearchPlaybookComposition,
    indices: &mut NormalizedLeafIndices,
) -> SearchPlaybookNormalizedComposition {
    match composition {
        SearchPlaybookComposition::Chain(children) => SearchPlaybookNormalizedComposition::Chain(
            children
                .iter()
                .map(|child| normalize_composition_references(child, indices))
                .collect(),
        ),
        SearchPlaybookComposition::Intersect(children) => {
            SearchPlaybookNormalizedComposition::Intersect(
                children
                    .iter()
                    .map(|child| normalize_composition_references(child, indices))
                    .collect(),
            )
        }
        SearchPlaybookComposition::Leaf(leaf) => {
            let (axis, block_index) = match leaf {
                SearchPlaybookLeaf::Rg(_) => {
                    take_index(&mut indices.rg, SearchPlaybookClauseAxis::Rg)
                }
                SearchPlaybookLeaf::Tantivy(_) => {
                    take_index(&mut indices.tantivy, SearchPlaybookClauseAxis::Tantivy)
                }
                SearchPlaybookLeaf::Topology(_) => {
                    take_index(&mut indices.topology, SearchPlaybookClauseAxis::Topology)
                }
                SearchPlaybookLeaf::Syntax(_) => {
                    take_index(&mut indices.syntax, SearchPlaybookClauseAxis::Syntax)
                }
                SearchPlaybookLeaf::NativeSyntax(_) => take_index(
                    &mut indices.native_syntax,
                    SearchPlaybookClauseAxis::NativeSyntax,
                ),
                SearchPlaybookLeaf::Graph(_) => {
                    take_index(&mut indices.graph, SearchPlaybookClauseAxis::Graph)
                }
            };
            SearchPlaybookNormalizedComposition::Leaf(SearchPlaybookClauseRef { axis, block_index })
        }
    }
}

fn take_index(
    index: &mut usize,
    axis: SearchPlaybookClauseAxis,
) -> (SearchPlaybookClauseAxis, usize) {
    let block_index = *index;
    *index = index.saturating_add(1);
    (axis, block_index)
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
    topology: Vec<TopologyOwnerMembershipBlock>,
    syntax: Vec<ProducerNativeBlock>,
    native_syntax: Vec<String>,
    graph: Vec<GraphNativeBlock>,
    clause_order: Vec<SearchPlaybookClauseRef>,
    graph_started: bool,
}

fn lower_v1_composition(
    composition: &SearchPlaybookComposition,
    state: &mut LoweringState,
) -> Result<(), ProgressiveSearchPlaybookError> {
    match composition {
        SearchPlaybookComposition::Chain(_) => {
            let mut sequence = Vec::new();
            flatten_chain(composition, &mut sequence);
            let Some((acquisition, tail)) = sequence.split_first() else {
                return Err(incomplete("chain requires a child"));
            };
            if matches!(
                acquisition,
                SearchPlaybookComposition::Intersect(_)
                    | SearchPlaybookComposition::Leaf(
                        SearchPlaybookLeaf::Rg(_)
                            | SearchPlaybookLeaf::Tantivy(_)
                            | SearchPlaybookLeaf::Topology(_)
                    )
            ) {
                lower_acquisition(acquisition, state)?;
            } else {
                lower_tail(acquisition, state)?;
            }
            for child in tail {
                lower_tail(child, state)?;
            }
        }
        SearchPlaybookComposition::Intersect(_) => lower_acquisition(composition, state)?,
        SearchPlaybookComposition::Leaf(
            leaf @ (SearchPlaybookLeaf::Rg(_)
            | SearchPlaybookLeaf::Tantivy(_)
            | SearchPlaybookLeaf::Topology(_)
            | SearchPlaybookLeaf::Syntax(_)
            | SearchPlaybookLeaf::NativeSyntax(_)),
        ) => lower_leaf(leaf, state)?,
        SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Graph(_)) => {
            return Err(invalid_parameter(
                "search-playbook-layout-invalid",
                "graph requires a preceding retrieval or structural predicate leaf",
            ));
        }
    }
    Ok(())
}

fn flatten_chain<'a>(
    composition: &'a SearchPlaybookComposition,
    sequence: &mut Vec<&'a SearchPlaybookComposition>,
) {
    match composition {
        SearchPlaybookComposition::Chain(children) => {
            for child in children {
                flatten_chain(child, sequence);
            }
        }
        other => sequence.push(other),
    }
}

fn lower_acquisition(
    composition: &SearchPlaybookComposition,
    state: &mut LoweringState,
) -> Result<(), ProgressiveSearchPlaybookError> {
    match composition {
        SearchPlaybookComposition::Intersect(children) => {
            if children.len() < 2 {
                return Err(incomplete(
                    "the acquisition intersect requires at least two children",
                ));
            }
            for child in children {
                lower_acquisition(child, state)?;
            }
        }
        SearchPlaybookComposition::Leaf(
            leaf @ (SearchPlaybookLeaf::Rg(_)
            | SearchPlaybookLeaf::Tantivy(_)
            | SearchPlaybookLeaf::Topology(_)),
        ) => lower_leaf(leaf, state)?,
        SearchPlaybookComposition::Leaf(_) | SearchPlaybookComposition::Chain(_) => {
            return Err(invalid_parameter(
                "search-playbook-layout-invalid",
                "V1 acquisition intersection admits only nested intersect, rg, tantivy, and topology",
            ));
        }
    }
    Ok(())
}

fn lower_tail(
    composition: &SearchPlaybookComposition,
    state: &mut LoweringState,
) -> Result<(), ProgressiveSearchPlaybookError> {
    match composition {
        SearchPlaybookComposition::Chain(children) => {
            for child in children {
                lower_tail(child, state)?;
            }
            Ok(())
        }
        SearchPlaybookComposition::Leaf(
            leaf @ (SearchPlaybookLeaf::Syntax(_)
            | SearchPlaybookLeaf::NativeSyntax(_)
            | SearchPlaybookLeaf::Graph(_)),
        ) => lower_leaf(leaf, state),
        SearchPlaybookComposition::Leaf(_) | SearchPlaybookComposition::Intersect(_) => {
            Err(invalid_parameter(
                "search-playbook-layout-invalid",
                "V1 chain tail admits only nested chain, syntax, native-syntax, and final graph",
            ))
        }
    }
}

fn lower_leaf(
    leaf: &SearchPlaybookLeaf,
    state: &mut LoweringState,
) -> Result<(), ProgressiveSearchPlaybookError> {
    match leaf {
        SearchPlaybookLeaf::Rg(argv) => {
            reject_after_graph(state)?;
            let block_index = state.rg.len();
            state.rg.push(argv.clone());
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Rg,
                block_index,
            });
        }
        SearchPlaybookLeaf::Tantivy(argv) => {
            reject_after_graph(state)?;
            let block_index = state.tantivy.len();
            state.tantivy.push(argv.clone());
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Tantivy,
                block_index,
            });
        }
        SearchPlaybookLeaf::Topology(block) => {
            reject_after_graph(state)?;
            let block_index = state.topology.len();
            state.topology.push(block.clone());
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Topology,
                block_index,
            });
        }
        SearchPlaybookLeaf::Syntax(block) => {
            reject_after_graph(state)?;
            let block_index = state.syntax.len();
            state.syntax.push(block.clone());
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Syntax,
                block_index,
            });
        }
        SearchPlaybookLeaf::NativeSyntax(selector) => {
            reject_after_graph(state)?;
            let block_index = state.native_syntax.len();
            state.native_syntax.push(selector.clone());
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::NativeSyntax,
                block_index,
            });
        }
        SearchPlaybookLeaf::Graph(block) => {
            if state.clause_order.is_empty() {
                return Err(ProgressiveSearchPlaybookError::InvalidClauseOrder(
                    "graph requires a preceding retrieval or syntax leaf".to_owned(),
                ));
            }
            state.graph_started = true;
            let block_index = state.graph.len();
            state.graph.push(block.clone());
            state.clause_order.push(SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Graph,
                block_index,
            });
        }
    }
    Ok(())
}

fn parse_composition(
    datum: &SchemeDatum,
    depth: usize,
    parsed_nodes: &mut usize,
) -> Result<SearchPlaybookComposition, ProgressiveSearchPlaybookError> {
    *parsed_nodes = parsed_nodes.saturating_add(1);
    if *parsed_nodes > SEARCH_PLAYBOOK_MAX_COMPOSITION_NODES
        || depth > SEARCH_PLAYBOOK_MAX_COMPOSITION_DEPTH
    {
        return Err(invalid_parameter(
            "search-playbook-composition-budget-exceeded",
            "Search Playbook Composition exceeds the V1 node or depth budget",
        ));
    }
    let form = operator_form(datum)?;
    let operator = symbol(form.first()).expect("operator_form checks the head");
    match operator {
        "chain" | "intersect" => {
            let minimum_children = if operator == "chain" { 1 } else { 2 };
            if form.len() <= minimum_children {
                return Err(incomplete(format!(
                    "{operator} requires at least {minimum_children} child{}",
                    if minimum_children == 1 { "" } else { "ren" }
                )));
            }
            let children = form[1..]
                .iter()
                .map(|child| parse_composition(child, depth + 1, parsed_nodes))
                .collect::<Result<Vec<_>, _>>()?;
            if operator == "chain" {
                Ok(SearchPlaybookComposition::Chain(children))
            } else {
                Ok(SearchPlaybookComposition::Intersect(children))
            }
        }
        "union" => Err(ProgressiveSearchPlaybookError::UnsupportedOption(
            "union".to_owned(),
        )),
        "rg" => {
            let argv = string_arguments(operator, &form[1..])?;
            require_nonempty(operator, &argv)?;
            validate_rg(&argv)?;
            Ok(SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Rg(
                argv,
            )))
        }
        "tantivy" => {
            let argv = string_arguments(operator, &form[1..])?;
            if argv.len() != 1 {
                return Err(incomplete("tantivy requires one query string"));
            }
            validate_tantivy(&argv[0])?;
            Ok(SearchPlaybookComposition::Leaf(
                SearchPlaybookLeaf::Tantivy(argv),
            ))
        }
        "topology" => Ok(SearchPlaybookComposition::Leaf(
            SearchPlaybookLeaf::Topology(crate::topology_owner::parse(&form[1..])?),
        )),
        "syntax" => {
            let (producer, argv) = producer_arguments(operator, &form[1..])?;
            if argv.len() != 1 {
                return Err(incomplete(
                    "syntax requires exactly one enhanced Tree-sitter Query string",
                ));
            }
            Ok(SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Syntax(
                ProducerNativeBlock { producer, argv },
            )))
        }
        "native-syntax" => {
            let argv = string_arguments(operator, &form[1..])?;
            if argv.len() != 1 || !valid_exact_selector(&argv[0]) {
                return Err(invalid_parameter(
                    "search-playbook-native-selector-invalid",
                    "native-syntax requires one canonical parser-owned selector",
                ));
            }
            Ok(SearchPlaybookComposition::Leaf(
                SearchPlaybookLeaf::NativeSyntax(argv[0].clone()),
            ))
        }
        "graph" => {
            let (language, argv) = producer_arguments(operator, &form[1..])?;
            if !matches!(language.as_str(), "gql" | "pgql") {
                return Err(invalid_parameter(
                    "search-playbook-graph-language-invalid",
                    "graph language must be gql or pgql",
                ));
            }
            Ok(SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Graph(
                GraphNativeBlock { language, argv },
            )))
        }
        other => Err(ProgressiveSearchPlaybookError::UnsupportedOption(
            other.to_owned(),
        )),
    }
}

fn admit_composition_budget(
    composition: &SearchPlaybookComposition,
) -> Result<(), ProgressiveSearchPlaybookError> {
    let metrics = composition.metrics();
    if metrics.nodes > SEARCH_PLAYBOOK_MAX_COMPOSITION_NODES
        || metrics.depth > SEARCH_PLAYBOOK_MAX_COMPOSITION_DEPTH
        || metrics.static_work > SEARCH_PLAYBOOK_MAX_STATIC_WORK
    {
        return Err(invalid_parameter(
            "search-playbook-composition-budget-exceeded",
            format!(
                "Search Playbook Composition budget exceeded: nodes={} depth={} staticWork={}",
                metrics.nodes, metrics.depth, metrics.static_work
            ),
        ));
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
    let analysis = agent_semantic_shell_parser::analyze_native_rg_argv(argv);
    if let Some(diagnostic) = analysis.diagnostics.first() {
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
    if analysis
        .search_roots
        .iter()
        .any(|root| root.token_index.is_some())
    {
        return Err(ProgressiveSearchPlaybookError::InvalidRg {
            reason_kind: "search-playbook-rg-scope-conflict",
            message: "rg must not carry a path; Search Playbook scope comes from the top-level registered WorkspaceId".to_owned(),
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
