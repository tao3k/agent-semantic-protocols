use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::fmt;

use agent_semantic_context_product::JSON_SAFE_INTEGER_MAX;
use agent_semantic_context_product::ProtocolId;
use agent_semantic_context_product::RouteEdge;
use agent_semantic_context_product::RouteExecutionMode;
use agent_semantic_context_product::RouteProgram;
use agent_semantic_context_product::RouteProposal;
use agent_semantic_context_product::RouteProposalExecutionGroup;
use agent_semantic_context_product::UncheckedContextProductStateV1;

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
    ExecutionGroupMismatch,
    ParallelDependency,
    BatchCapability,
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
                Self::ExecutionGroupMismatch => "route execution groups are invalid",
                Self::ParallelDependency => {
                    "parallel route group contains a dependency path"
                }
                Self::BatchCapability => {
                    "batch route group is not covered by one provider capability"
                }
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
        || program.catalog_digest != proposal.catalog_digest
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
    let proposal_groups =
        validate_proposal_execution_groups(&proposal_nodes, &proposal_edges, proposal)?;

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
            || stage.catalog_id != node.catalog_id
            || stage.input_template_digest != node.input_template_digest
            || stage.covers_obligation_ids != node.covers_obligation_ids
            || stage.required_closure != node.required_closure
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
    validate_program_execution_groups(
        &proposal_groups,
        &program_stages,
        &program_edges,
        &node_to_stage,
        program,
    )?;
    validate_proposal_join_lowering(proposal, program, &node_to_stage)?;
    validate_joins(&program_stages, &program_edges, program)?;
    Ok(())
}

fn validate_budget(
    proposal: &RouteProposal,
    program: &RouteProgram,
) -> Result<(), RouteValidationError> {
    let proposed = &proposal.budget_proposal;
    let admitted = &program.budget_limit;
    if proposed.max_commands.get() == 0
        || proposed.max_elapsed_ms.get() == 0
        || proposed.max_packet_bytes.get() == 0
        || proposed.max_choice_depth.get() == 0
        || proposed.max_parallel.get() == 0
        || proposed.max_aggregate_provider_latency_ms.get() == 0
        || proposed.max_parent_visible_bytes.get() == 0
        || admitted.max_commands.get() == 0
        || admitted.max_elapsed_ms.get() == 0
        || admitted.max_packet_bytes.get() == 0
        || admitted.max_choice_depth.get() == 0
        || admitted.max_parallel.get() == 0
        || admitted.max_aggregate_provider_latency_ms.get() == 0
        || admitted.max_parent_visible_bytes.get() == 0
        || proposed.max_commands.get() > JSON_SAFE_INTEGER_MAX
        || proposed.max_elapsed_ms.get() > JSON_SAFE_INTEGER_MAX
        || proposed.max_packet_bytes.get() > JSON_SAFE_INTEGER_MAX
        || proposed.max_choice_depth.get() > JSON_SAFE_INTEGER_MAX
        || proposed.max_parallel.get() > 10
        || proposed.max_aggregate_provider_latency_ms.get() > JSON_SAFE_INTEGER_MAX
        || proposed.max_parent_visible_bytes.get() > JSON_SAFE_INTEGER_MAX
        || admitted.max_commands.get() > proposed.max_commands.get()
        || admitted.max_elapsed_ms.get() > proposed.max_elapsed_ms.get()
        || admitted.max_packet_bytes.get() > proposed.max_packet_bytes.get()
        || admitted.max_choice_depth.get() > proposed.max_choice_depth.get()
        || admitted.max_parallel.get() > proposed.max_parallel.get()
        || admitted.max_aggregate_provider_latency_ms.get()
            > proposed.max_aggregate_provider_latency_ms.get()
        || admitted.max_parent_visible_bytes.get() > proposed.max_parent_visible_bytes.get()
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

fn validate_proposal_execution_groups<'a>(
    nodes: &BTreeMap<&'a ProtocolId, &'a agent_semantic_context_product::RouteNode>,
    edges: &BTreeSet<(&'a ProtocolId, &'a ProtocolId)>,
    proposal: &'a RouteProposal,
) -> Result<BTreeMap<&'a ProtocolId, &'a RouteProposalExecutionGroup>, RouteValidationError> {
    let groups: BTreeMap<_, _> = proposal
        .execution_groups
        .iter()
        .map(|group| (&group.group_id, group))
        .collect();
    if groups.is_empty() || groups.len() != proposal.execution_groups.len() {
        return Err(RouteValidationError::ExecutionGroupMismatch);
    }

    let mut assigned = BTreeSet::new();
    for group in &proposal.execution_groups {
        let group_nodes = unique_refs(&group.node_ids)?;
        if group_nodes.is_empty()
            || group_nodes
                .iter()
                .any(|node_id| !nodes.contains_key(*node_id))
            || group_nodes.iter().any(|node_id| !assigned.insert(*node_id))
        {
            return Err(RouteValidationError::ExecutionGroupMismatch);
        }
        validate_execution_mode(
            group.mode,
            group.node_ids.len(),
            group.max_parallel,
            group.batch_capability_ref.as_ref(),
            group.independence_proof_ref.as_ref(),
        )?;

        validate_group_dependencies(group.mode, &group.node_ids, edges)?;

        if group.mode == RouteExecutionMode::Batch {
            let first = nodes
                .get(&group.node_ids[0])
                .ok_or(RouteValidationError::MissingReference)?;
            if group.node_ids.iter().skip(1).any(|node_id| {
                nodes.get(node_id).is_none_or(|node| {
                    node.provider_id != first.provider_id || node.catalog_id != first.catalog_id
                })
            }) {
                return Err(RouteValidationError::BatchCapability);
            }
        }
    }
    if assigned.len() != nodes.len() {
        return Err(RouteValidationError::ExecutionGroupMismatch);
    }
    Ok(groups)
}

fn validate_program_execution_groups<'a>(
    proposal_groups: &BTreeMap<&'a ProtocolId, &'a RouteProposalExecutionGroup>,
    stages: &BTreeMap<&'a ProtocolId, &'a agent_semantic_context_product::RouteStage>,
    edges: &BTreeSet<(&'a ProtocolId, &'a ProtocolId)>,
    node_to_stage: &BTreeMap<&'a ProtocolId, &'a ProtocolId>,
    program: &'a RouteProgram,
) -> Result<(), RouteValidationError> {
    let groups: BTreeMap<_, _> = program
        .execution_groups
        .iter()
        .map(|group| (&group.group_id, group))
        .collect();
    if groups.len() != program.execution_groups.len() || groups.len() != proposal_groups.len() {
        return Err(RouteValidationError::ExecutionGroupMismatch);
    }

    let mut assigned = BTreeSet::new();
    for group in &program.execution_groups {
        let proposal_group = proposal_groups
            .get(&group.group_id)
            .ok_or(RouteValidationError::ExecutionGroupMismatch)?;
        let expected_stage_ids: Vec<_> = proposal_group
            .node_ids
            .iter()
            .map(|node_id| {
                node_to_stage
                    .get(node_id)
                    .copied()
                    .ok_or(RouteValidationError::StageLowering)
            })
            .collect::<Result<_, _>>()?;
        if expected_stage_ids != group.stage_ids.iter().collect::<Vec<_>>()
            || group.mode != proposal_group.mode
            || group.join_policy != proposal_group.join_policy
            || group.max_parallel != proposal_group.max_parallel
            || group.derivation_receipt_ref != proposal_group.derivation_receipt_ref
            || group.batch_capability_ref != proposal_group.batch_capability_ref
            || group.independence_proof_ref != proposal_group.independence_proof_ref
        {
            return Err(RouteValidationError::ExecutionGroupMismatch);
        }

        let group_stages = unique_refs(&group.stage_ids)?;
        if group_stages
            .iter()
            .any(|stage_id| !stages.contains_key(*stage_id))
            || group_stages
                .iter()
                .any(|stage_id| !assigned.insert(*stage_id))
        {
            return Err(RouteValidationError::ExecutionGroupMismatch);
        }
        validate_execution_mode(
            group.mode,
            group.stage_ids.len(),
            group.max_parallel,
            group.batch_capability_ref.as_ref(),
            group.independence_proof_ref.as_ref(),
        )?;
        validate_group_dependencies(group.mode, &group.stage_ids, edges)?;
    }
    if assigned.len() != stages.len() {
        return Err(RouteValidationError::ExecutionGroupMismatch);
    }
    Ok(())
}

fn validate_execution_mode(
    mode: RouteExecutionMode,
    item_count: usize,
    max_parallel: u64,
    batch_capability_ref: Option<&ProtocolId>,
    independence_proof_ref: Option<&ProtocolId>,
) -> Result<(), RouteValidationError> {
    match mode {
        RouteExecutionMode::Serial => {
            if item_count == 0
                || max_parallel != 1
                || batch_capability_ref.is_some()
                || independence_proof_ref.is_some()
            {
                return Err(RouteValidationError::ExecutionGroupMismatch);
            }
        }
        RouteExecutionMode::Batch => {
            if item_count < 2
                || max_parallel != 1
                || batch_capability_ref.is_none()
                || independence_proof_ref.is_some()
            {
                return Err(RouteValidationError::BatchCapability);
            }
        }
        RouteExecutionMode::Parallel => {
            if item_count < 2
                || max_parallel < 2
                || max_parallel > item_count as u64
                || batch_capability_ref.is_some()
                || independence_proof_ref.is_none()
            {
                return Err(RouteValidationError::ExecutionGroupMismatch);
            }
        }
    }
    Ok(())
}

fn contains_dependency_path<'a>(
    ids: &[ProtocolId],
    edges: &BTreeSet<(&'a ProtocolId, &'a ProtocolId)>,
) -> bool {
    ids.iter().enumerate().any(|(index, from)| {
        ids.iter()
            .skip(index + 1)
            .any(|to| has_path(from, to, edges) || has_path(to, from, edges))
    })
}

fn validate_group_dependencies<'a>(
    mode: RouteExecutionMode,
    ids: &[ProtocolId],
    edges: &BTreeSet<(&'a ProtocolId, &'a ProtocolId)>,
) -> Result<(), RouteValidationError> {
    if matches!(
        mode,
        RouteExecutionMode::Batch | RouteExecutionMode::Parallel
    ) && contains_dependency_path(ids, edges)
    {
        return Err(RouteValidationError::ParallelDependency);
    }
    Ok(())
}

fn has_path<'a>(
    from: &ProtocolId,
    to: &ProtocolId,
    edges: &BTreeSet<(&'a ProtocolId, &'a ProtocolId)>,
) -> bool {
    let mut ready = vec![from.clone()];
    let mut visited = BTreeSet::new();
    while let Some(current) = ready.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }
        for (edge_from, edge_to) in edges {
            if **edge_from == current {
                if **edge_to == *to {
                    return true;
                }
                ready.push((*edge_to).clone());
            }
        }
    }
    false
}

fn validate_proposal_join_lowering(
    proposal: &RouteProposal,
    program: &RouteProgram,
    node_to_stage: &BTreeMap<&ProtocolId, &ProtocolId>,
) -> Result<(), RouteValidationError> {
    let program_joins: BTreeMap<_, _> = program
        .joins
        .iter()
        .map(|join| (&join.join_id, join))
        .collect();
    if program_joins.len() != program.joins.len() || program_joins.len() != proposal.joins.len() {
        return Err(RouteValidationError::JoinMismatch);
    }
    for proposal_join in &proposal.joins {
        let program_join = program_joins
            .get(&proposal_join.join_id)
            .ok_or(RouteValidationError::JoinMismatch)?;
        let required_stage_ids: Vec<_> = proposal_join
            .required_node_ids
            .iter()
            .map(|node_id| {
                node_to_stage
                    .get(node_id)
                    .copied()
                    .ok_or(RouteValidationError::MissingReference)
            })
            .collect::<Result<_, _>>()?;
        let continuation_stage_id = node_to_stage
            .get(&proposal_join.continuation_node_id)
            .copied()
            .ok_or(RouteValidationError::MissingReference)?;
        if required_stage_ids != program_join.required_stage_ids.iter().collect::<Vec<_>>()
            || continuation_stage_id != &program_join.continuation_stage_id
            || proposal_join.policy != program_join.policy
        {
            return Err(RouteValidationError::JoinMismatch);
        }
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

#[cfg(test)]
#[path = "../tests/unit/route_validation.rs"]
mod tests;
