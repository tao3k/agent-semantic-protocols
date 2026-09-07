// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Bounded MRR/Ascent closure over parser-owned Project Topology edges.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;

use meta_relational_reasoning::{
    Atom, CandidateIdentities, ClosureStatus, DeductionLimits, DeductionPlan, DerivationId,
    EntityId, EvidenceCompleteness, Fact, FactId, FactProvenance, FactValidity, GenerationId,
    MrrEngine, ReasoningBundle, ReasoningBundleDeclaration, RelationAuthority, RelationCardinality,
    RelationContext, RelationField, RelationId, RelationSchema, Rule, RuleId, RulePack, RulePackId,
    Term, Value, ValueType, Variable,
};

const ID_DOMAIN: &str = "agent-semantic-topology:project-topology-closure";
const RECEIPT_SCHEMA_ID: &str = "agent.semantic-protocols.project-topology-inference-receipt";
const RECEIPT_SCHEMA_VERSION: &str = "1";
const RULE_PACK_IDENTITY: &str = "project-topology-reachability";

/// One admitted MRR program whose exact bundle is executed for topology
/// closure. Its digest is derived from canonical bundle bytes; callers cannot
/// supply a display digest independently from executable semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyInferenceProgram {
    bundle: ReasoningBundle,
    direct_relation: RelationId,
    reachable_relation: RelationId,
    rule_pack: RulePackId,
    base_rule: RuleId,
    transitive_rule: RuleId,
    digest: String,
}

impl ProjectTopologyInferenceProgram {
    /// Builds the standard topology reachability prelude as an admitted MRR
    /// bundle. Project-owned AOT bundles enter through `admit` with the same
    /// explicit relation and rule identities.
    pub fn standard() -> Result<Self, ProjectTopologyClosureError> {
        let ids = ClosureIdentities::new();
        Self::admit(
            topology_program_bundle(&ids)?,
            ids.direct_relation,
            ids.reachable_relation,
            ids.rule_pack,
            ids.base_rule,
            ids.transitive_rule,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn admit(
        bundle: ReasoningBundle,
        direct_relation: RelationId,
        reachable_relation: RelationId,
        rule_pack: RulePackId,
        base_rule: RuleId,
        transitive_rule: RuleId,
    ) -> Result<Self, ProjectTopologyClosureError> {
        bundle
            .validate()
            .map_err(|cause| error("topology-mrr-bundle-invalid", format!("{cause:?}")))?;
        for relation in [direct_relation, reachable_relation] {
            if !bundle
                .relations()
                .iter()
                .any(|schema| schema.id() == relation)
            {
                return Err(error(
                    "topology-mrr-program-relation-missing",
                    "topology MRR program omits a required relation",
                ));
            }
        }
        let pack = bundle
            .declaration()
            .rule_packs
            .iter()
            .find(|candidate| candidate.id() == rule_pack)
            .ok_or_else(|| {
                error(
                    "topology-mrr-program-rule-pack-missing",
                    "topology MRR program omits the selected rule pack",
                )
            })?;
        for rule in [base_rule, transitive_rule] {
            if !pack.rules().iter().any(|candidate| candidate.id() == rule) {
                return Err(error(
                    "topology-mrr-program-rule-missing",
                    "topology MRR program omits a required closure rule",
                ));
            }
        }
        let digest = format!(
            "blake3-256:{}",
            blake3::hash(bundle.encode_canonical()).to_hex()
        );
        Ok(Self {
            bundle,
            direct_relation,
            reachable_relation,
            rule_pack,
            base_rule,
            transitive_rule,
            digest,
        })
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// One direct relationship emitted by a native parser or admitted document grammar.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTopologyDirectEdge {
    id: String,
    relation: String,
    from: String,
    to: String,
}

impl ProjectTopologyDirectEdge {
    pub fn new(
        id: impl Into<String>,
        relation: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Result<Self, ProjectTopologyClosureError> {
        let edge = Self {
            id: id.into(),
            relation: relation.into(),
            from: from.into(),
            to: to.into(),
        };
        if edge.id.is_empty()
            || edge.relation.is_empty()
            || edge.from.is_empty()
            || edge.to.is_empty()
        {
            return Err(error(
                "topology-direct-edge-invalid",
                "edge identity, relation, and endpoints must be non-empty",
            ));
        }
        Ok(edge)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn to(&self) -> &str {
        &self.to
    }
}

/// Hard bounds applied before or during MRR evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectTopologyClosureLimits {
    max_input_edges: NonZeroUsize,
    max_relationships: NonZeroUsize,
    max_results: NonZeroUsize,
}

impl ProjectTopologyClosureLimits {
    pub fn new(
        max_input_edges: usize,
        max_relationships: usize,
        max_results: usize,
    ) -> Result<Self, ProjectTopologyClosureError> {
        Ok(Self {
            max_input_edges: non_zero(max_input_edges)?,
            max_relationships: non_zero(max_relationships)?,
            max_results: non_zero(max_results)?,
        })
    }
}

/// One MRR-derived reachable relationship with its complete parser-premise support.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTopologyRelationship {
    from: String,
    to: String,
    premise_edge_ids: Vec<String>,
}

impl ProjectTopologyRelationship {
    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn to(&self) -> &str {
        &self.to
    }

    pub fn premise_edge_ids(&self) -> &[String] {
        &self.premise_edge_ids
    }
}

/// Identity-complete result of independently evaluated and admitted MRR closure.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTopologyInferenceReceipt {
    schema_id: &'static str,
    schema_version: &'static str,
    state: &'static str,
    generation_identity: String,
    rule_pack_identity: &'static str,
    input_edges: Vec<ProjectTopologyDirectEdge>,
    relationships: Vec<ProjectTopologyRelationship>,
    mrr_closure_digest: String,
    mrr_materialization_digest: String,
    receipt_digest: String,
    terminal: ProjectTopologyInferenceTerminal,
}

impl ProjectTopologyInferenceReceipt {
    pub const fn state(&self) -> &'static str {
        self.state
    }

    pub const fn input_edge_count(&self) -> usize {
        self.input_edges.len()
    }

    pub const fn relationship_count(&self) -> usize {
        self.relationships.len()
    }

    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }

    pub fn as_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("typed topology inference receipt is serializable")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectTopologyInferenceTerminal {
    state: &'static str,
    terminal_count: u8,
    reason_kind: Option<&'static str>,
}

/// Complete deterministic closure generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyClosure {
    relationships: Vec<ProjectTopologyRelationship>,
    receipt: ProjectTopologyInferenceReceipt,
}

impl ProjectTopologyClosure {
    pub fn relationships(&self) -> &[ProjectTopologyRelationship] {
        &self.relationships
    }

    pub const fn receipt(&self) -> &ProjectTopologyInferenceReceipt {
        &self.receipt
    }
}

/// Runtime-independent builder for the reusable topology closure generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyClosureBuilder {
    limits: ProjectTopologyClosureLimits,
    program: Arc<ProjectTopologyInferenceProgram>,
}

impl ProjectTopologyClosureBuilder {
    pub fn new(
        limits: ProjectTopologyClosureLimits,
        program: Arc<ProjectTopologyInferenceProgram>,
    ) -> Self {
        Self { limits, program }
    }

    pub fn build(
        &self,
        generation: &str,
        direct_edges: &[ProjectTopologyDirectEdge],
    ) -> Result<ProjectTopologyClosure, ProjectTopologyClosureError> {
        if generation.is_empty() {
            return Err(error(
                "topology-generation-identity-invalid",
                "generation identity must be non-empty",
            ));
        }
        let mut edge_ids = BTreeSet::new();
        for edge in direct_edges {
            if !edge_ids.insert(edge.id.as_str()) {
                return Err(error(
                    "topology-direct-edge-duplicate",
                    format!("duplicate parser edge {}", edge.id),
                ));
            }
        }

        let generation_id = canonical_id::<GenerationId>(&format!("generation:{generation}"))?;
        let mut support_names = BTreeMap::new();
        let facts = direct_edges
            .iter()
            .map(|edge| {
                let fact_id =
                    canonical_id::<FactId>(&format!("source-fact:{}:{}", edge.relation, edge.id))?;
                support_names.insert(fact_id, edge.id.clone());
                Ok(Fact::new(
                    fact_id,
                    self.program.direct_relation,
                    vec![
                        Value::String(edge.from.clone()),
                        Value::String(edge.to.clone()),
                    ],
                    RelationContext::new(
                        generation_id,
                        RelationAuthority::Entity(canonical_id("authority:parser-owned-topology")?),
                        FactProvenance::Source(canonical_id("authority:parser-owned-topology")?),
                        EvidenceCompleteness::Complete,
                        FactValidity::Valid,
                    ),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let engine = MrrEngine::builder()
            .with_bundle(self.program.bundle.clone())
            .build()
            .map_err(|cause| error("topology-mrr-engine-invalid", format!("{cause:?}")))?
            .admit(facts)
            .map_err(|cause| error("topology-mrr-fact-admission-failed", format!("{cause:?}")))?;
        let closure = engine
            .derive(
                DeductionPlan::transitive_closure(
                    self.program.direct_relation,
                    self.program.reachable_relation,
                    self.program.rule_pack,
                    self.program.base_rule,
                    self.program.transitive_rule,
                ),
                generation_id,
                DeductionLimits::new(
                    self.limits.max_input_edges,
                    self.limits.max_relationships,
                    self.limits.max_results,
                ),
            )
            .map_err(|cause| error("topology-closure-evaluation-failed", cause.to_string()))?;
        if closure.status() != ClosureStatus::Complete {
            return Err(error(
                "topology-closure-incomplete",
                "MRR closure output was truncated",
            ));
        }

        let assigned = closure
            .candidates()
            .iter()
            .map(|candidate| {
                let [from, to] = candidate.values();
                let key = format!(
                    "{}:{}:{:?}",
                    string_value(from)?,
                    string_value(to)?,
                    candidate.support()
                );
                Ok(CandidateIdentities::new(
                    canonical_id::<FactId>(&format!("derived-fact:{key}"))?,
                    canonical_id::<DerivationId>(&format!("derivation:{key}"))?,
                ))
            })
            .collect::<Result<Vec<_>, ProjectTopologyClosureError>>()?;
        let previous_generation = canonical_id::<GenerationId>(&format!("previous:{generation}"))?;
        let materialized = engine
            .materialize(&closure, previous_generation, generation_id, &assigned)
            .map_err(|cause| error("topology-closure-admission-failed", format!("{cause:?}")))?;

        let mut relationships = closure
            .candidates()
            .iter()
            .map(|candidate| {
                let [from, to] = candidate.values();
                let mut premise_edge_ids = candidate
                    .support()
                    .iter()
                    .map(|fact| {
                        support_names.get(fact).cloned().ok_or_else(|| {
                            error(
                                "topology-closure-premise-unresolved",
                                "MRR receipt references an unknown parser premise",
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                premise_edge_ids.sort();
                Ok(ProjectTopologyRelationship {
                    from: string_value(from)?.to_owned(),
                    to: string_value(to)?.to_owned(),
                    premise_edge_ids,
                })
            })
            .collect::<Result<Vec<_>, ProjectTopologyClosureError>>()?;
        relationships.sort_by(|left, right| {
            (&left.from, &left.to, &left.premise_edge_ids).cmp(&(
                &right.from,
                &right.to,
                &right.premise_edge_ids,
            ))
        });
        let mrr_closure_digest = format!(
            "blake3-256:{}",
            blake3::hash(closure.digest().as_bytes()).to_hex()
        );
        let mrr_materialization_digest = format!(
            "blake3-256:{}",
            blake3::hash(materialized.receipt().digest()).to_hex()
        );
        let mut input_edges = direct_edges.to_vec();
        input_edges.sort_by(|left, right| left.id.cmp(&right.id));
        let receipt_digest = topology_receipt_digest(
            generation,
            &input_edges,
            &relationships,
            &mrr_closure_digest,
            &mrr_materialization_digest,
        );
        Ok(ProjectTopologyClosure {
            receipt: ProjectTopologyInferenceReceipt {
                schema_id: RECEIPT_SCHEMA_ID,
                schema_version: RECEIPT_SCHEMA_VERSION,
                state: "admitted",
                generation_identity: generation.to_owned(),
                rule_pack_identity: RULE_PACK_IDENTITY,
                input_edges,
                relationships: relationships.clone(),
                mrr_closure_digest,
                mrr_materialization_digest,
                receipt_digest,
                terminal: ProjectTopologyInferenceTerminal {
                    state: "admitted",
                    terminal_count: 1,
                    reason_kind: None,
                },
            },
            relationships,
        })
    }
}

fn topology_receipt_digest(
    generation: &str,
    input_edges: &[ProjectTopologyDirectEdge],
    relationships: &[ProjectTopologyRelationship],
    mrr_closure_digest: &str,
    mrr_materialization_digest: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hash_text(&mut hasher, RECEIPT_SCHEMA_ID);
    hash_text(&mut hasher, RECEIPT_SCHEMA_VERSION);
    hash_text(&mut hasher, generation);
    hash_text(&mut hasher, RULE_PACK_IDENTITY);
    for edge in input_edges {
        hash_text(&mut hasher, &edge.id);
        hash_text(&mut hasher, &edge.relation);
        hash_text(&mut hasher, &edge.from);
        hash_text(&mut hasher, &edge.to);
    }
    for relationship in relationships {
        hash_text(&mut hasher, &relationship.from);
        hash_text(&mut hasher, &relationship.to);
        for premise in &relationship.premise_edge_ids {
            hash_text(&mut hasher, premise);
        }
    }
    hash_text(&mut hasher, mrr_closure_digest);
    hash_text(&mut hasher, mrr_materialization_digest);
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn hash_text(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

struct ClosureIdentities {
    direct_relation: RelationId,
    reachable_relation: RelationId,
    rule_pack: RulePackId,
    base_rule: RuleId,
    transitive_rule: RuleId,
}

impl ClosureIdentities {
    fn new() -> Self {
        Self {
            direct_relation: canonical_id("relation:direct").expect("constant identity"),
            reachable_relation: canonical_id("relation:reachable").expect("constant identity"),
            rule_pack: canonical_id("rule-pack:topology-reachability").expect("constant identity"),
            base_rule: canonical_id("rule:direct-is-reachable").expect("constant identity"),
            transitive_rule: canonical_id("rule:reachable-transitive").expect("constant identity"),
        }
    }
}

fn topology_program_bundle(
    ids: &ClosureIdentities,
) -> Result<ReasoningBundle, ProjectTopologyClosureError> {
    let variable = |name: &str| {
        Variable::new(name).map(Term::Variable).ok_or_else(|| {
            error(
                "topology-mrr-rule-invalid",
                format!("invalid variable name {name}"),
            )
        })
    };
    let atom = |relation, names: &[&str]| -> Result<Atom, ProjectTopologyClosureError> {
        Ok(Atom {
            relation,
            terms: names
                .iter()
                .map(|name| variable(name))
                .collect::<Result<Vec<_>, _>>()?,
        })
    };
    let schema = |relation, predicate: &str| {
        RelationSchema::new(
            relation,
            predicate,
            vec![
                RelationField::new("from", ValueType::String).expect("constant field"),
                RelationField::new("to", ValueType::String).expect("constant field"),
            ],
            RelationCardinality::ManyToMany,
        )
        .map_err(|cause| error("topology-mrr-schema-invalid", format!("{cause:?}")))
    };
    let rules = vec![
        Rule::new(
            ids.base_rule,
            atom(ids.reachable_relation, &["x", "y"])?,
            vec![atom(ids.direct_relation, &["x", "y"])?],
        )
        .map_err(|cause| error("topology-mrr-rule-invalid", format!("{cause:?}")))?,
        Rule::new(
            ids.transitive_rule,
            atom(ids.reachable_relation, &["x", "z"])?,
            vec![
                atom(ids.reachable_relation, &["x", "y"])?,
                atom(ids.direct_relation, &["y", "z"])?,
            ],
        )
        .map_err(|cause| error("topology-mrr-rule-invalid", format!("{cause:?}")))?,
    ];
    ReasoningBundle::admit(ReasoningBundleDeclaration {
        relations: vec![
            schema(ids.direct_relation, "topology_direct_edge")?,
            schema(ids.reachable_relation, "topology_reachable")?,
        ],
        rule_packs: vec![RulePack::new(ids.rule_pack, rules)],
        ..ReasoningBundleDeclaration::default()
    })
    .map_err(|cause| error("topology-mrr-bundle-invalid", format!("{cause:?}")))
}

fn string_value(value: &Value) -> Result<&str, ProjectTopologyClosureError> {
    match value {
        Value::String(value) => Ok(value),
        _ => Err(error(
            "topology-mrr-value-invalid",
            "topology closure returned a non-string endpoint",
        )),
    }
}

fn non_zero(value: usize) -> Result<NonZeroUsize, ProjectTopologyClosureError> {
    NonZeroUsize::new(value).ok_or_else(|| {
        error(
            "topology-closure-limit-invalid",
            "topology closure limits must be non-zero",
        )
    })
}

fn canonical_id<T>(suffix: &str) -> Result<T, ProjectTopologyClosureError>
where
    T: CanonicalIdentity,
{
    T::from_bytes(format!("{ID_DOMAIN}:{suffix}"))
}

trait CanonicalIdentity: Sized {
    fn from_bytes(value: String) -> Result<Self, ProjectTopologyClosureError>;
}

macro_rules! canonical_identity {
    ($($identity:ty),+ $(,)?) => {$ (
        impl CanonicalIdentity for $identity {
            fn from_bytes(value: String) -> Result<Self, ProjectTopologyClosureError> {
                Self::from_canonical_bytes(value).map_err(|cause| {
                    error("topology-mrr-identity-invalid", format!("{cause:?}"))
                })
            }
        }
    )+ };
}

canonical_identity!(
    RelationId,
    RulePackId,
    RuleId,
    EntityId,
    FactId,
    DerivationId,
    GenerationId,
);

/// Fail-closed Project Topology closure construction error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyClosureError {
    reason_kind: &'static str,
    message: String,
}

impl ProjectTopologyClosureError {
    pub const fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for ProjectTopologyClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for ProjectTopologyClosureError {}

fn error(reason_kind: &'static str, message: impl Into<String>) -> ProjectTopologyClosureError {
    ProjectTopologyClosureError {
        reason_kind,
        message: message.into(),
    }
}
