// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
//! Private bridge from resident provider sets to Data aggregation and MRR inference.
use mrr_data_core::{
    DataPooSearchCandidateBranch, DataSearchCandidateComposition, DataSearchSourceBinding,
    compose_poo_data_search_candidates,
};
use mrr_search_kernel::{
    GenerationId, PooSearchPlan, PooSearchRole, SearchFrameworkLimits, SearchFrameworkStatus,
    compile_poo_search_plan, evaluate_poo_search_factors,
};
use std::{collections::BTreeSet, num::NonZeroUsize};
/// Configure the explicitly published MRR Scheme owner for this process.
/// The executable is admitted by the Runtime bundle owner before this call.
pub fn configure_data_search_execution(executable: &std::path::Path) -> Result<(), String> {
    mrr_search_kernel::configure_native_worker(executable).map_err(|error| {
        format!("owner=data_search_execution reasonKind=mrr-worker-configuration-failed {error}")
    })
}

pub struct DataSearchLeaf {
    pub identity: String,
    pub owners: BTreeSet<String>,
    pub complete: bool,
    pub truncated: bool,
}
pub struct DataSearchCompositionInput<'a> {
    pub project_id: &'a str,
    pub workspace_id: &'a str,
    pub runtime_generation: &'a str,
    pub content_generation: &'a str,
    pub expected_content_generation: &'a str,
    pub resident_view_digest: &'a str,
    pub composition_abi: &'a str,
    pub query_digest: &'a str,
    pub mode: DataSearchCandidateComposition,
    pub leaves: Vec<DataSearchLeaf>,
    pub max_observations: NonZeroUsize,
}
#[derive(Debug)]
pub struct DataSearchCompositionOutput {
    pub owners: BTreeSet<String>,
    pub receipt: serde_json::Value,
}
/// Aggregate actual owner sets and infer their native MRR causal factor graph.
pub fn compose_resident_data_search(
    input: DataSearchCompositionInput<'_>,
) -> Result<DataSearchCompositionOutput, String> {
    if input.expected_content_generation != input.content_generation {
        return Err("reasonKind=search-data-stale-content-generation".to_owned());
    }
    for identity in [
        input.project_id,
        input.workspace_id,
        input.runtime_generation,
        input.content_generation,
        input.resident_view_digest,
        input.composition_abi,
        input.query_digest,
    ] {
        if identity.is_empty() || identity.trim() != identity {
            return Err("reasonKind=search-data-invalid-source-binding".to_owned());
        }
    }
    let scope = serde_json::to_string(&(input.project_id, input.workspace_id))
        .map_err(|error| error.to_string())?;
    let generation_bytes = serde_json::to_vec(&(
        "asp.resident.data-search-generation.v1",
        &scope,
        input.runtime_generation,
        input.content_generation,
        input.resident_view_digest,
        input.composition_abi,
    ))
    .map_err(|error| error.to_string())?;
    let generation =
        GenerationId::from_canonical_bytes(generation_bytes).map_err(|error| error.to_string())?;
    let binding = DataSearchSourceBinding::new(
        scope,
        input.content_generation.to_owned(),
        input.resident_view_digest.to_owned(),
        input.composition_abi.to_owned(),
        generation,
    )
    .map_err(|error| format!("reasonKind=search-data-binding-invalid {error}"))?;
    let branch_count = input.leaves.len();
    let mut identities = BTreeSet::new();
    if input.leaves.iter().any(|leaf| {
        leaf.identity.is_empty()
            || leaf.identity.trim() != leaf.identity
            || !identities.insert(leaf.identity.clone())
    }) {
        return Err("reasonKind=search-poo-leaf-identity-invalid".to_owned());
    }
    let stage_names = input
        .leaves
        .iter()
        .enumerate()
        .map(|(index, leaf)| format!("leaf-{index}-{}", blake3::hash(leaf.identity.as_bytes())))
        .collect::<Vec<_>>();
    let stage =
        |name: String, role, input_domain: String, output_domain: String| PooSearchPlan::Stage {
            name,
            role,
            input_domain,
            output_domain,
        };
    let leaves = stage_names
        .iter()
        .map(|name| {
            stage(
                name.clone(),
                PooSearchRole::Acquisition,
                "workspace".into(),
                "candidate-set".into(),
            )
        })
        .collect::<Vec<_>>();
    let plan = if branch_count == 1 {
        PooSearchPlan::Chain {
            name: "single".into(),
            children: vec![
                leaves[0].clone(),
                stage(
                    "merge".into(),
                    PooSearchRole::Refinement,
                    "candidate-set".into(),
                    "candidate-set".into(),
                ),
            ],
        }
    } else {
        PooSearchPlan::Merge {
            name: "combined".into(),
            parallel: Box::new(PooSearchPlan::Parallel {
                name: "leaves".into(),
                children: leaves,
            }),
            stage_name: "merge".into(),
            role: PooSearchRole::Refinement,
            output_domain: "candidate-set".into(),
        }
    };
    let strategy_name = format!("query-{}", blake3::hash(input.query_digest.as_bytes()));
    let projection = compile_poo_search_plan(&strategy_name, generation, &plan)
        .map_err(|error| format!("reasonKind=search-poo-plan-rejected {error}"))?;
    let branches = input
        .leaves
        .into_iter()
        .enumerate()
        .map(|(index, leaf)| DataPooSearchCandidateBranch {
            stage_name: stage_names[index].clone(),
            binding: binding.clone(),
            candidates: leaf.owners,
            complete: leaf.complete,
            truncated: leaf.truncated,
        })
        .collect::<Vec<_>>();
    let data = compose_poo_data_search_candidates(
        &binding,
        input.mode,
        &projection,
        "merge",
        &branches,
        input.max_observations,
    )
    .map_err(|error| format!("reasonKind=search-data-candidate-composition-invalid {error}"))?;
    for (owner, candidate) in data.candidate_owners() {
        data.verify_candidate(&binding, owner, *candidate)
            .map_err(|error| {
                format!("reasonKind=search-data-candidate-identity-invalid {error}")
            })?;
    }
    let nonzero = |count: usize| NonZeroUsize::new(count.max(1)).expect("positive resource bound");
    let influence_bound = data
        .observations()
        .len()
        .checked_mul(data.factors().len())
        .ok_or_else(|| "reasonKind=search-data-influence-budget-overflow".to_owned())?;
    let limits = SearchFrameworkLimits::new(
        nonzero(data.factors().len()),
        nonzero(data.edges().len()),
        input.max_observations,
        nonzero(influence_bound),
        nonzero(influence_bound),
    );
    let reasoning = evaluate_poo_search_factors(&projection, data.observations(), limits)
        .map_err(|error| format!("reasonKind=search-data-native-inference-failed {error}"))?;
    if reasoning.generation() != generation || reasoning.status() != SearchFrameworkStatus::Complete
    {
        return Err("reasonKind=search-data-native-inference-incomplete".to_owned());
    }
    let mode = match input.mode {
        DataSearchCandidateComposition::Single => "single",
        DataSearchCandidateComposition::RankJoin => "rankJoin",
        DataSearchCandidateComposition::Intersect => "intersect",
    };
    let receipt = serde_json::json!({
        "schemaId": "agent.semantic-protocols.search-data-composition-receipt",
        "schemaVersion": "1", "scope": binding.scope(), "generationIdentity": generation.to_string(),
        "pooDagDigest": format!("blake3-256:{}", blake3::hash(projection.dag_receipt().as_bytes())),
        "pooFactorIds": projection.factors().iter().map(|factor| factor.id().to_string()).collect::<Vec<_>>(),
        "pooEdges": projection.edges().iter().map(|edge| [edge.from().to_string(), edge.to().to_string()]).collect::<Vec<_>>(),
        "sourceDigest": binding.source_digest(), "residentViewDigest": binding.resident_view_digest(),
        "compositionAbiDigest": binding.composition_abi(), "mode": mode,
        "compositionIdentity": data.composition_id().to_string(),
        "branchCompleteness": data.branch_completeness(),
        "branchCount": branch_count, "mergedOwnerCount": data.merged_candidates().len(),
        "observationCount": data.observations().len(),
        "mergedOwnerIds": data.merged_candidates().iter().take(32).collect::<Vec<_>>(),
        "mergedOwnerIdsTruncated": data.merged_candidates().len() > 32, "nativeReasoningDigest": reasoning.digest().to_string(),
    });
    // Independent evidence for MRR's Lean gate; the existing receipt stays unchanged.
    let witness_complete = branches.len() <= 256
        && branches
            .iter()
            .map(|branch| branch.candidates.len())
            .sum::<usize>()
            <= 256
        && projection.paths().len() <= 256
        && data.observations().len() <= 256
        && reasoning.influences().len() <= 256
        && data.merged_candidates().len() <= 32;
    let witness = witness_complete.then(|| serde_json::json!({
        "branches": branches.iter().enumerate().map(|(index, branch)| serde_json::json!({
            "factor": projection.factor_by_name(&stage_names[index]).expect("admitted POO stage").id().to_string(),
            "owners": branch.candidates, "generation": branch.binding.generation().to_string(),
            "scope": branch.binding.scope(), "source": branch.binding.source_digest(),
            "resident": branch.binding.resident_view_digest(), "abi": branch.binding.composition_abi(),
            "complete": branch.complete, "truncated": branch.truncated,
        })).collect::<Vec<_>>(),
        "paths": projection.paths().iter().map(|(from, to, distance)|
            serde_json::json!([from.to_string(), to.to_string(), distance])).collect::<Vec<_>>(),
        "observations": data.observations().iter().map(|observation| serde_json::json!({
            "id": observation.id().to_string(), "candidate": observation.candidate().to_string(),
            "factor": observation.factor().to_string(), "generation": observation.generation().to_string(),
            "position": observation.logical_position(),
            "parents": observation.causal_parents().iter().map(ToString::to_string).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "influences": reasoning.influences().iter().map(|influence| serde_json::json!({
            "candidate": influence.candidate().to_string(), "factor": influence.factor().to_string(),
            "support": influence.support_event().to_string(),
            "path": influence.factor_path().iter().map(ToString::to_string).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    }));
    eprintln!(
        "[runtime-search-math-witness] witness={}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.search-execution-witness", "schemaVersion": "1",
            "complete": witness_complete, "compositionReceipt": receipt, "witness": witness,
        })
    );
    eprintln!(
        "[runtime-search-candidate-witness] witness={}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.search-candidate-witness", "schemaVersion": "1",
            "complete": witness_complete, "compositionReceipt": receipt,
            "ownerCandidates": witness_complete.then(|| data.candidate_owners().iter()
                .map(|(owner, candidate)| (owner, candidate.to_string())).collect::<Vec<_>>()),
        })
    );
    // Separate Version 1 order evidence leaves the existing receipts unchanged.
    let order_evidence = projection.role_evidence();
    let factor = |input: &str| {
        projection
            .factor_by_input(input)
            .expect("source-owned role root is an admitted factor")
            .id()
            .to_string()
    };
    eprintln!(
        "[runtime-search-order-witness] witness={}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.search-order-witness", "schemaVersion": "1",
            "complete": order_evidence.is_some(), "compositionReceipt": receipt,
            "runtimeAlgorithm": "gerbil.runtime.c4",
            "roleGraph": order_evidence.map(|evidence| &evidence.graph),
            "roleRoots": order_evidence.map(|evidence| evidence.roots.iter().map(|(input, root)|
                serde_json::json!([factor(input), root])).collect::<Vec<_>>()),
            "runtimeOrders": order_evidence.map(|evidence| evidence.runtime_orders.iter().map(|(input, order)|
                serde_json::json!([factor(input), order])).collect::<Vec<_>>()),
        })
    );
    Ok(DataSearchCompositionOutput {
        owners: data.merged_candidates().clone(),
        receipt,
    })
}
#[cfg(test)]
#[path = "../tests/unit/data_search_composition.rs"]
mod tests;
