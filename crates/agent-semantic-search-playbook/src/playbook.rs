// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure typed Search Playbook V1 validation and normalized request lowering.
//!
//! Marlin's real Gerbil Scheme module system evaluates search compositions and
//! projects them across the native `RootedSchemeObject` ABI directly into
//! [`SearchPlaybookPlanInput`]. No pseudo S-expression string parsing exists here.

use crate::model::{
    GraphNativeBlock, ProducerNativeBlock, ProgressiveSearchPlaybookError,
    ProgressiveSearchPlaybookRequest, SEARCH_PLAYBOOK_MAX_COMPOSITION_DEPTH,
    SEARCH_PLAYBOOK_MAX_COMPOSITION_NODES, SEARCH_PLAYBOOK_MAX_STATIC_WORK,
    SearchPlaybookClauseAxis, SearchPlaybookClauseRef, SearchPlaybookComposition,
    SearchPlaybookLeaf, SearchPlaybookNormalizedComposition, SearchPlaybookPlanInput,
    TopologyOwnerMembershipBlock,
};

/// Admit a pure typed Search Playbook plan projected from Marlin's Gerbil
/// Scheme module system via native `RootedSchemeObject` ABI.
pub fn admit_search_playbook_plan(
    input: SearchPlaybookPlanInput,
) -> Result<ProgressiveSearchPlaybookRequest, ProgressiveSearchPlaybookError> {
    if let Some(workspace) = &input.workspace
        && !valid_registered_name(workspace)
    {
        return Err(invalid_parameter(
            "search-playbook-workspace-identity-invalid",
            "workspace requires one registered WorkspaceId, not a path",
        ));
    }
    let language = normalize_producer_axis(&input.languages)?;
    let documents = normalize_producer_axis(&input.documents)?;
    if language.is_none() && documents.is_none() {
        return Err(incomplete("at least one producer axis is required"));
    }
    admit_composition_budget(&input.composition)?;
    validate_composition_leaves(&input.composition)?;
    admit_validated_composition(input.workspace, language, documents, input.composition)
}

fn normalize_producer_axis(
    values: &[String],
) -> Result<Option<String>, ProgressiveSearchPlaybookError> {
    if values.is_empty() {
        return Ok(None);
    }
    if values.iter().any(|value| !valid_registered_name(value)) {
        return Err(invalid_parameter(
            "search-playbook-producer-expression-invalid",
            "producer values must be registered-name symbols",
        ));
    }
    Ok(Some(values.join("|")))
}

fn validate_composition_leaves(
    composition: &SearchPlaybookComposition,
) -> Result<(), ProgressiveSearchPlaybookError> {
    match composition {
        SearchPlaybookComposition::Chain(children) => {
            if children.is_empty() {
                return Err(incomplete("chain requires at least 1 child"));
            }
            children.iter().try_for_each(validate_composition_leaves)
        }
        SearchPlaybookComposition::Intersect(children) => {
            if children.len() < 2 {
                return Err(incomplete("intersect requires at least 2 children"));
            }
            children.iter().try_for_each(validate_composition_leaves)
        }
        SearchPlaybookComposition::Leaf(leaf) => validate_leaf_value(leaf),
    }
}

fn validate_leaf_value(leaf: &SearchPlaybookLeaf) -> Result<(), ProgressiveSearchPlaybookError> {
    match leaf {
        SearchPlaybookLeaf::Rg(argv) => {
            require_nonempty("rg", argv)?;
            validate_rg(argv)
        }
        SearchPlaybookLeaf::Tantivy(argv) => {
            if argv.len() != 1 {
                return Err(incomplete("tantivy requires one query string"));
            }
            validate_tantivy(&argv[0])
        }
        SearchPlaybookLeaf::Topology(block) => {
            crate::topology_owner::validate_block(block.clone()).map(|_| ())
        }
        SearchPlaybookLeaf::Syntax(block) => {
            if !valid_registered_name(&block.producer) {
                return Err(invalid_parameter(
                    "search-playbook-producer-expression-invalid",
                    "invalid syntax producer",
                ));
            }
            if block.argv.len() != 1 {
                return Err(incomplete(
                    "syntax requires exactly one enhanced Tree-sitter Query string",
                ));
            }
            Ok(())
        }
        SearchPlaybookLeaf::NativeSyntax(selector) => {
            if !valid_exact_selector(selector) {
                return Err(invalid_parameter(
                    "search-playbook-native-selector-invalid",
                    "native-syntax requires one canonical parser-owned selector",
                ));
            }
            Ok(())
        }
        SearchPlaybookLeaf::Graph(block) => {
            if !matches!(block.language.as_str(), "gql" | "pgql") {
                return Err(invalid_parameter(
                    "search-playbook-graph-language-invalid",
                    "graph language must be gql or pgql",
                ));
            }
            require_nonempty("graph", &block.argv)
        }
    }
}

fn admit_validated_composition(
    workspace: Option<String>,
    language: Option<String>,
    documents: Option<String>,
    composition: SearchPlaybookComposition,
) -> Result<ProgressiveSearchPlaybookRequest, ProgressiveSearchPlaybookError> {
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
                children.iter().try_for_each(|child| visit(child, seen))
            }
            SearchPlaybookComposition::Leaf(leaf) => {
                if seen.contains(&leaf) {
                    return Err(invalid_parameter(
                        "search-playbook-redundant-predicate",
                        "Search Playbook rejects byte-identical predicate leaves as redundant work",
                    ));
                }
                seen.push(leaf);
                Ok(())
            }
        }
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
            children
                .iter()
                .try_for_each(|child| lower_acquisition(child, state))
        }
        SearchPlaybookComposition::Leaf(
            leaf @ (SearchPlaybookLeaf::Rg(_)
            | SearchPlaybookLeaf::Tantivy(_)
            | SearchPlaybookLeaf::Topology(_)),
        ) => lower_leaf(leaf, state),
        SearchPlaybookComposition::Leaf(_) | SearchPlaybookComposition::Chain(_) => {
            Err(invalid_parameter(
                "search-playbook-layout-invalid",
                "V1 acquisition intersection admits only nested intersect, rg, tantivy, and topology",
            ))
        }
    }
}

fn lower_tail(
    composition: &SearchPlaybookComposition,
    state: &mut LoweringState,
) -> Result<(), ProgressiveSearchPlaybookError> {
    match composition {
        SearchPlaybookComposition::Chain(children) => children
            .iter()
            .try_for_each(|child| lower_tail(child, state)),
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
    use crate::native_rg::NativeRgDiagnosticKind;
    let analysis = crate::native_rg::analyze_native_rg_argv(argv);
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
    let analysis = crate::tantivy_query::analyze_tantivy_query(expression);
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
