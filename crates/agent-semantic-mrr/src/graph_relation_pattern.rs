// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fmt;

use gql_ir::{EdgeDirection, Expression, GraphPatternElement};

/// Direction admitted by the executable V1 GQL/PGQL relation adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphRelationDirection {
    Out,
    In,
    Undirected,
}

/// Compiler-derived relation plan. Source text is deliberately absent: each
/// retained field affects execution against one admitted relation graph.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRelationPattern {
    pub left_binding: String,
    pub left_kind: String,
    pub relation: String,
    pub direction: GraphRelationDirection,
    pub right_binding: String,
    pub right_kind: String,
    pub projected_bindings: Vec<String>,
}

/// Compile one bounded, unfiltered relation pattern through the pinned GQL IR.
/// The caller may require a projected node kind such as ASP Search's `Owner`.
pub fn compile_graph_relation_pattern_v1(
    language: &str,
    argv: &[String],
    required_projected_node_kind: Option<&str>,
) -> Result<GraphRelationPattern, GraphRelationPatternError> {
    if !matches!(language, "gql" | "pgql") {
        return Err(error(format!(
            "graph language is not registered by the V1 relation adapter: {language}"
        )));
    }
    let catalog = gql_catalog::Catalog::new(
        gql_catalog::CatalogName("asp-workspace".to_owned()),
        Vec::new(),
        Vec::new(),
    );
    let source = argv.join(" ");
    let compilation = gql_compiler::Compiler.compile("search-playbook.gql", &source, &catalog);
    if !compilation.analysis.diagnostics.is_empty() {
        return Err(error(format!(
            "{language} graph query is invalid: {:?}",
            compilation.analysis.diagnostics
        )));
    }
    let query = compilation
        .analysis
        .ir
        .ok_or_else(|| error(format!("{language} graph query produced no executable IR")))?;
    if query.matches.len() != 1
        || !query.optional_matches.is_empty()
        || !query.filters.is_empty()
        || !query.let_bindings.is_empty()
        || !query.for_bindings.is_empty()
        || !query.mutations.is_empty()
        || query.projection_quantifier != gql_ir::SetQuantifier::All
        || query.is_finish
        || !query.group_by.is_empty()
        || !query.set_operations.is_empty()
        || query.limit.is_some()
        || !query.order_by.is_empty()
        || query.offset.is_some()
    {
        return Err(error(
            "V1 Graph fan-in admits one unfiltered MATCH relation pattern",
        ));
    }
    let graph_match = &query.matches[0];
    if graph_match.paths.len() != 1 || graph_match.paths[0].elements.len() != 3 {
        return Err(error(
            "V1 Graph fan-in requires MATCH (left)-[:RELATION]->(right)",
        ));
    }
    let elements = &graph_match.paths[0].elements;
    let (
        GraphPatternElement::Node(left),
        GraphPatternElement::Edge(edge),
        GraphPatternElement::Node(right),
    ) = (&elements[0], &elements[1], &elements[2])
    else {
        return Err(error("V1 Graph fan-in requires a node-edge-node path"));
    };
    if edge.labels.len() != 1
        || edge.quantifier.is_some()
        || !edge.properties.is_empty()
        || edge.predicate.is_some()
        || !left.properties.is_empty()
        || left.predicate.is_some()
        || !right.properties.is_empty()
        || right.predicate.is_some()
        || left.labels.len() != 1
        || right.labels.len() != 1
    {
        return Err(error(
            "V1 Graph fan-in requires labelled endpoints, one edge label, and no inline predicates",
        ));
    }
    let left_binding = left
        .binding
        .clone()
        .ok_or_else(|| error("V1 Graph fan-in requires a left endpoint binding"))?;
    let right_binding = right
        .binding
        .clone()
        .ok_or_else(|| error("V1 Graph fan-in requires a right endpoint binding"))?;
    if left_binding == right_binding {
        return Err(error("V1 Graph fan-in requires distinct endpoint bindings"));
    }
    let projected_bindings = query
        .projection
        .iter()
        .map(
            |projection| match (&projection.expression, &projection.alias) {
                (Expression::Binding(binding), None)
                    if binding == &left_binding || binding == &right_binding =>
                {
                    Ok(binding.clone())
                }
                _ => Err(error(
                    "V1 Graph fan-in RETURN admits only unaliased endpoint bindings",
                )),
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    if projected_bindings.is_empty() {
        return Err(error(
            "V1 Graph fan-in requires at least one projected endpoint",
        ));
    }
    let left_kind = left.labels[0].clone();
    let right_kind = right.labels[0].clone();
    if let Some(required_kind) = required_projected_node_kind {
        let required_kind_is_projected = (left_kind.eq_ignore_ascii_case(required_kind)
            && projected_bindings.contains(&left_binding))
            || (right_kind.eq_ignore_ascii_case(required_kind)
                && projected_bindings.contains(&right_binding));
        if !required_kind_is_projected {
            return Err(error(format!(
                "V1 Graph fan-in must project an {required_kind} endpoint"
            )));
        }
    }
    Ok(GraphRelationPattern {
        left_binding,
        left_kind,
        relation: edge.labels[0].clone(),
        direction: match edge.direction {
            EdgeDirection::Out => GraphRelationDirection::Out,
            EdgeDirection::In => GraphRelationDirection::In,
            EdgeDirection::Undirected => GraphRelationDirection::Undirected,
        },
        right_binding,
        right_kind,
        projected_bindings,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphRelationPatternError {
    message: String,
}

impl fmt::Display for GraphRelationPatternError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GraphRelationPatternError {}

fn error(message: impl Into<String>) -> GraphRelationPatternError {
    GraphRelationPatternError {
        message: message.into(),
    }
}
