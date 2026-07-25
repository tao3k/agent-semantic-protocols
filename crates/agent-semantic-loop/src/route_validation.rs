use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use agent_semantic_context_product::{
    JSON_SAFE_INTEGER_MAX, RouteEdge, RouteProgram, RouteProposal, UncheckedContextProductStateV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteValidationError {
    StateBinding,
    ProposalDigest,
    ProgramDigest,
    GraphDigest,
    Budget,
    ObligationCoverage,
    DuplicateId,
    MissingReference,
    DependencyMismatch,
    StageLowering,
    JoinMismatch,
    Cycle,
    EvidencePredicate,
}

impl fmt::Display for RouteValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}",
            match self {
                Self::StateBinding => "route state binding mismatch",
                Self::ProposalDigest => "route proposal digest mismatch",
                Self::ProgramDigest => "route program digest mismatch",
                Self::GraphDigest => "route graph digest mismatch",
                Self::Budget => "route budget is invalid",
                Self::ObligationCoverage => "route obligation coverage is invalid",
                Self::DuplicateId => "route contains a duplicate identity",
                Self::MissingReference => "route contains a missing reference",
                Self::DependencyMismatch => "proposal dependency forms disagree",
                Self::StageLowering => "program does not faithfully lower proposal nodes",
                Self::JoinMismatch => "program join does not match its continuation inputs",
                Self::Cycle => "route graph is cyclic",
                Self::EvidencePredicate => "route evidence predicate is invalid",
            }
        )
    }
}

impl std::error::Error for RouteValidationError {}

pub(crate) fn validate_route(
    state: &UncheckedContextProductStateV1,
    proposal: &RouteProposal,
    program: &RouteProgram,
) -> Result<(), RouteValidationError> {
    if proposal.run_id != state.run_id
        || proposal.based_on_revision != state.revision
        || proposal.context_binding_digest != state.context.binding_digest
        || program.run_id != state.run_id
        || program.proposal_id != proposal.proposal_id
        || program.admitted_at_revision != state.revision
        || program.context_binding_digest != state.context.binding_digest
    {
        return Err(RouteValidationError::StateBinding);
    }
    if proposal.proposal_digest != proposal.recompute_proposal_digest() {
        return Err(RouteValidationError::ProposalDigest);
    }
    if program.graph_digest != program.recompute_graph_digest() {
        return Err(RouteValidationError::GraphDigest);
    }
    if program.program_digest != program.recompute_program_digest() {
        return Err(RouteValidationError::ProgramDigest);
    }
    validate_budget(proposal, program)?;

    let state_obligations: BTreeSet<_> = state
        .obligations
        .iter()
        .map(|obligation| &obligation.obligation_id)
        .collect();
    let proposal_obligations = unique_refs(&proposal.obligation_ids)?;
    if proposal_obligations.is_empty() || !proposal_obligations.is_subset(&state_obligations) {
        return Err(RouteValidationError::ObligationCoverage);
    }

    let proposal_nodes: BTreeMap<_, _> = proposal
        .nodes
        .iter()
        .map(|node| (&node.node_id, node))
        .collect();
    if proposal_nodes.len() != proposal.nodes.len() || proposal_nodes.is_empty() {
        return Err(RouteValidationError::DuplicateId);
    }
    for node in &proposal.nodes {
        let covered = unique_refs(&node.covers_obligation_ids)?;
        if covered.is_empty() || !covered.is_subset(&proposal_obligations) {
            return Err(RouteValidationError::ObligationCoverage);
        }
        validate_evidence_predicate(&node.evidence_predicate)?;
    }

    let dependency_edges: BTreeSet<_> = proposal
        .nodes
        .iter()
        .flat_map(|node| {
            node.depends_on_node_ids
                .iter()
                .map(move |dependency| (dependency, &node.node_id))
        })
        .collect();
    let proposal_edges = edge_set(&proposal.edges)?;
    if dependency_edges != proposal_edges {
        return Err(RouteValidationError::DependencyMismatch);
    }
    validate_dag(proposal_nodes.keys().copied().collect(), &proposal_edges)?;

    let program_stages: BTreeMap<_, _> = program
        .stages
        .iter()
        .map(|stage| (&stage.stage_id, stage))
        .collect();
    if program_stages.len() != program.stages.len() || program_stages.len() != proposal_nodes.len()
    {
        return Err(RouteValidationError::StageLowering);
    }
    let mut node_to_stage = BTreeMap::new();
    for stage in &program.stages {
        let node = proposal_nodes
            .get(&stage.proposal_node_id)
            .ok_or(RouteValidationError::MissingReference)?;
        if node_to_stage
            .insert(&stage.proposal_node_id, &stage.stage_id)
            .is_some()
            || stage.provider_id != node.provider_id
            || stage.operation != node.operation
            || stage.input_template_digest != node.input_template_digest
            || stage.covers_obligation_ids != node.covers_obligation_ids
            || stage.evidence_predicate != node.evidence_predicate
        {
            return Err(RouteValidationError::StageLowering);
        }
        validate_evidence_predicate(&stage.evidence_predicate)?;
    }
    let expected_program_edges: BTreeSet<_> = dependency_edges
        .iter()
        .map(|(from, to)| {
            (
                *node_to_stage
                    .get(from)
                    .expect("dependency source was lowered"),
                *node_to_stage
                    .get(to)
                    .expect("dependency target was lowered"),
            )
        })
        .collect();
    let program_edges = edge_set(&program.edges)?;
    if program_edges != expected_program_edges {
        return Err(RouteValidationError::StageLowering);
    }
    validate_dag(program_stages.keys().copied().collect(), &program_edges)?;
    validate_joins(&program_stages, &program_edges, program)?;
    Ok(())
}

fn validate_budget(
    proposal: &RouteProposal,
    program: &RouteProgram,
) -> Result<(), RouteValidationError> {
    let proposed = &proposal.budget_proposal;
    let admitted = &program.budget_limit;
    if proposed.max_commands == 0
        || proposed.max_elapsed_ms == 0
        || proposed.max_packet_bytes == 0
        || admitted.max_commands == 0
        || admitted.max_elapsed_ms == 0
        || admitted.max_packet_bytes == 0
        || proposed.max_commands > JSON_SAFE_INTEGER_MAX
        || proposed.max_elapsed_ms > JSON_SAFE_INTEGER_MAX
        || proposed.max_packet_bytes > JSON_SAFE_INTEGER_MAX
        || admitted.max_commands > proposed.max_commands
        || admitted.max_elapsed_ms > proposed.max_elapsed_ms
        || admitted.max_packet_bytes > proposed.max_packet_bytes
    {
        return Err(RouteValidationError::Budget);
    }
    Ok(())
}

fn validate_evidence_predicate(
    predicate: &agent_semantic_context_product::EvidencePredicate,
) -> Result<(), RouteValidationError> {
    if predicate.max_results == 0
        || predicate.max_results > JSON_SAFE_INTEGER_MAX
        || predicate.accepted_schema_ids.is_empty()
        || predicate.required_fields.is_empty()
        || unique_refs(&predicate.accepted_schema_ids)?.len() != predicate.accepted_schema_ids.len()
        || unique_refs(&predicate.required_fields)?.len() != predicate.required_fields.len()
    {
        return Err(RouteValidationError::EvidencePredicate);
    }
    Ok(())
}

fn validate_joins<'a>(
    stages: &BTreeMap<
        &'a agent_semantic_context_product::ProtocolId,
        &'a agent_semantic_context_product::RouteStage,
    >,
    edges: &BTreeSet<(
        &'a agent_semantic_context_product::ProtocolId,
        &'a agent_semantic_context_product::ProtocolId,
    )>,
    program: &'a RouteProgram,
) -> Result<(), RouteValidationError> {
    let mut joins_by_continuation = BTreeMap::new();
    for join in &program.joins {
        if joins_by_continuation
            .insert(&join.continuation_stage_id, join)
            .is_some()
            || !stages.contains_key(&join.continuation_stage_id)
            || join.required_stage_ids.is_empty()
            || join
                .required_stage_ids
                .iter()
                .any(|stage_id| !stages.contains_key(stage_id))
        {
            return Err(RouteValidationError::JoinMismatch);
        }
        let incoming: BTreeSet<_> = edges
            .iter()
            .filter_map(|(from, to)| (*to == &join.continuation_stage_id).then_some(*from))
            .collect();
        let required = unique_refs(&join.required_stage_ids)?;
        if incoming != required || required.contains(&join.continuation_stage_id) {
            return Err(RouteValidationError::JoinMismatch);
        }
    }
    for continuation in stages.keys() {
        let incoming_count = edges.iter().filter(|(_, to)| to == continuation).count();
        if (incoming_count > 1) != joins_by_continuation.contains_key(continuation) {
            return Err(RouteValidationError::JoinMismatch);
        }
    }
    Ok(())
}

fn edge_set(
    edges: &[RouteEdge],
) -> Result<
    BTreeSet<(
        &agent_semantic_context_product::ProtocolId,
        &agent_semantic_context_product::ProtocolId,
    )>,
    RouteValidationError,
> {
    let set: BTreeSet<_> = edges.iter().map(|edge| (&edge.from, &edge.to)).collect();
    if set.len() != edges.len() || set.iter().any(|(from, to)| from == to) {
        return Err(RouteValidationError::DuplicateId);
    }
    Ok(set)
}

fn validate_dag<'a>(
    ids: BTreeSet<&'a agent_semantic_context_product::ProtocolId>,
    edges: &BTreeSet<(
        &'a agent_semantic_context_product::ProtocolId,
        &'a agent_semantic_context_product::ProtocolId,
    )>,
) -> Result<(), RouteValidationError> {
    if edges
        .iter()
        .any(|(from, to)| !ids.contains(from) || !ids.contains(to))
    {
        return Err(RouteValidationError::MissingReference);
    }
    let mut indegree: BTreeMap<_, usize> = ids.iter().map(|id| (*id, 0)).collect();
    let mut successors: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (from, to) in edges {
        *indegree.get_mut(to).expect("checked edge target") += 1;
        successors.entry(*from).or_default().push(*to);
    }
    let mut ready: VecDeque<_> = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
        .collect();
    let mut visited = 0;
    while let Some(node) = ready.pop_front() {
        visited += 1;
        for successor in successors.get(node).into_iter().flatten() {
            let degree = indegree.get_mut(successor).expect("checked successor");
            *degree -= 1;
            if *degree == 0 {
                ready.push_back(successor);
            }
        }
    }
    if visited != ids.len() {
        return Err(RouteValidationError::Cycle);
    }
    Ok(())
}

fn unique_refs(
    ids: &[agent_semantic_context_product::ProtocolId],
) -> Result<BTreeSet<&agent_semantic_context_product::ProtocolId>, RouteValidationError> {
    let unique: BTreeSet<_> = ids.iter().collect();
    if unique.len() != ids.len() {
        return Err(RouteValidationError::DuplicateId);
    }
    Ok(unique)
}
