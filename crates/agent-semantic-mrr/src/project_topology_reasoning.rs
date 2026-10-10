// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Bounded relation-sensitive Ascent rules for the standard topology prelude.
//!
//! This module is deliberately inside the MRR maintenance facade. Downstream
//! topology code submits already admitted source facts and receives a complete
//! deterministic candidate receipt; it does not depend on Ascent directly.

use std::collections::BTreeSet;
use std::fmt;

use ascent::ascent;

const PROGRAM_ID: &str = "mrr.topology.domain.v1";
const RULE_ID: &str = "mrr.topology.domain.depends-on-config.v1";
const OUTPUT_RELATION: &str = "DEPENDS_ON_CONFIG";

/// One source-authority edge admitted by the topology layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyReasoningInputV1 {
    pub fact_id: String,
    pub relation: String,
    pub from: String,
    pub to: String,
}

/// Hard pre-execution and output bounds for the relation-sensitive program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectTopologyReasoningLimitsV1 {
    pub max_input_facts: usize,
    pub max_candidates: usize,
}

/// One intensional edge and the exact source facts that witness its rule body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyReasoningCandidateV1 {
    relation: &'static str,
    from: String,
    to: String,
    rule_id: &'static str,
    premise_fact_ids: Vec<String>,
}

impl ProjectTopologyReasoningCandidateV1 {
    #[must_use]
    pub const fn relation(&self) -> &'static str {
        self.relation
    }

    #[must_use]
    pub fn from(&self) -> &str {
        &self.from
    }

    #[must_use]
    pub fn to(&self) -> &str {
        &self.to
    }

    #[must_use]
    pub const fn rule_id(&self) -> &'static str {
        self.rule_id
    }

    #[must_use]
    pub fn premise_fact_ids(&self) -> &[String] {
        &self.premise_fact_ids
    }
}

/// Complete fixed-point receipt from the bounded standard domain program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyReasoningReceiptV1 {
    candidates: Vec<ProjectTopologyReasoningCandidateV1>,
    digest: String,
}

impl ProjectTopologyReasoningReceiptV1 {
    #[must_use]
    pub fn candidates(&self) -> &[ProjectTopologyReasoningCandidateV1] {
        &self.candidates
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Evaluate the synchronized Lean rule
/// `CALLS(caller, callee) + READS_CONFIG(callee, config)`
/// `=> DEPENDS_ON_CONFIG(caller, config)`.
///
/// Relation names are canonical source-owned vocabulary and are matched
/// exactly. Other relations remain available to generic topology reachability
/// but can never impersonate a premise of this semantic rule.
pub fn evaluate_project_topology_v1(
    input: &[ProjectTopologyReasoningInputV1],
    limits: ProjectTopologyReasoningLimitsV1,
) -> Result<ProjectTopologyReasoningReceiptV1, ProjectTopologyReasoningErrorV1> {
    if limits.max_input_facts == 0 || limits.max_candidates == 0 {
        return Err(error("topology reasoning limits must be non-zero"));
    }
    if input.len() > limits.max_input_facts {
        return Err(error(format!(
            "topology reasoning input budget exceeded: required={} limit={}",
            input.len(),
            limits.max_input_facts
        )));
    }
    let unique_ids = input
        .iter()
        .map(|edge| edge.fact_id.as_str())
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != input.len() || input.iter().any(invalid_input) {
        return Err(error(
            "topology reasoning facts require unique non-empty identities and endpoints plus canonical uppercase relations",
        ));
    }

    let calls = input
        .iter()
        .filter(|edge| edge.relation == "CALLS")
        .map(|edge| (edge.from.clone(), edge.to.clone(), edge.fact_id.clone()))
        .collect::<Vec<_>>();
    let reads_config = input
        .iter()
        .filter(|edge| edge.relation == "READS_CONFIG")
        .map(|edge| (edge.from.clone(), edge.to.clone(), edge.fact_id.clone()))
        .collect::<Vec<_>>();

    ascent! {
        relation calls(String, String, String);
        relation reads_config(String, String, String);
        relation depends_on_config(String, String);

        depends_on_config(caller, config) <--
            calls(caller, callee, _call_fact),
            reads_config(callee, config, _config_fact);
    }
    let mut program = AscentProgram {
        calls,
        reads_config,
        ..AscentProgram::default()
    };
    program.run();

    let mut candidates = program
        .depends_on_config
        .into_iter()
        .map(|(from, to)| {
            let mut witnesses = Vec::new();
            for call in input
                .iter()
                .filter(|call| call.relation == "CALLS" && call.from == from)
            {
                for config in input.iter().filter(|config| {
                    config.relation == "READS_CONFIG" && config.from == call.to && config.to == to
                }) {
                    witnesses.push(vec![call.fact_id.clone(), config.fact_id.clone()]);
                }
            }
            witnesses.sort();
            let premise_fact_ids = witnesses
                .into_iter()
                .next()
                .expect("every Ascent join has a reconstructible source witness");
            ProjectTopologyReasoningCandidateV1 {
                relation: OUTPUT_RELATION,
                from,
                to,
                rule_id: RULE_ID,
                premise_fact_ids,
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (&left.relation, &left.from, &left.to, &left.premise_fact_ids).cmp(&(
            &right.relation,
            &right.from,
            &right.to,
            &right.premise_fact_ids,
        ))
    });
    if candidates.len() > limits.max_candidates {
        return Err(error(format!(
            "topology reasoning candidate budget exceeded: required={} limit={}",
            candidates.len(),
            limits.max_candidates
        )));
    }

    let digest = receipt_digest(input, &candidates);
    Ok(ProjectTopologyReasoningReceiptV1 { candidates, digest })
}

fn invalid_input(edge: &ProjectTopologyReasoningInputV1) -> bool {
    edge.fact_id.is_empty()
        || edge.from.is_empty()
        || edge.to.is_empty()
        || edge.relation.is_empty()
        || !edge
            .relation
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn receipt_digest(
    input: &[ProjectTopologyReasoningInputV1],
    candidates: &[ProjectTopologyReasoningCandidateV1],
) -> String {
    let mut input = input.to_vec();
    input.sort_by(|left, right| left.fact_id.cmp(&right.fact_id));
    let mut hasher = blake3::Hasher::new();
    for value in [PROGRAM_ID, RULE_ID] {
        hash_text(&mut hasher, value);
    }
    for edge in input {
        for value in [edge.fact_id, edge.relation, edge.from, edge.to] {
            hash_text(&mut hasher, &value);
        }
    }
    for candidate in candidates {
        for value in [
            candidate.relation,
            candidate.from(),
            candidate.to(),
            candidate.rule_id,
        ] {
            hash_text(&mut hasher, value);
        }
        for premise in &candidate.premise_fact_ids {
            hash_text(&mut hasher, premise);
        }
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn hash_text(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyReasoningErrorV1 {
    message: String,
}

impl fmt::Display for ProjectTopologyReasoningErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ProjectTopologyReasoningErrorV1 {}

fn error(message: impl Into<String>) -> ProjectTopologyReasoningErrorV1 {
    ProjectTopologyReasoningErrorV1 {
        message: message.into(),
    }
}
