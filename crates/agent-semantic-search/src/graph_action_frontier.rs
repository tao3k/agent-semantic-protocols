use std::collections::BTreeMap;

/// A dependency node admitted by the typed graph owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DependencyActionNodeV1<'a> {
    /// Stable graph node identity.
    pub id: &'a str,
    /// Canonical dependency name.
    pub dependency: &'a str,
}

/// Derives dependency actions exclusively from targets of typed `matches`
/// edges.
///
/// Dependency nodes without a matching graph edge are intentionally ignored.
/// The graph owner must establish routing evidence before an action can enter
/// the frontier.
pub fn matched_dependency_action_targets<'a>(
    dependency_nodes: impl IntoIterator<Item = DependencyActionNodeV1<'a>>,
    matched_edge_targets: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let dependencies = dependency_nodes
        .into_iter()
        .map(|node| (node.id, node.dependency))
        .collect::<BTreeMap<_, _>>();
    matched_edge_targets
        .into_iter()
        .filter_map(|target| dependencies.get(target))
        .fold(Vec::new(), |mut targets, dependency| {
            if !targets.iter().any(|target| target == dependency) {
                targets.push((*dependency).to_string());
            }
            targets
        })
}
