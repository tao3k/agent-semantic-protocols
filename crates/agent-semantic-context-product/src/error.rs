// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::ProtocolId;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    InvalidProtocolId(String),
    InvalidDigest(String),
    InvalidSchemaId(String),
    InvalidSchemaVersion(u32),
    InvalidCanonicalizationProfile(String),
    UnsafeInteger(&'static str),
    StateChainMismatch,
    InvalidActiveProgram,
    InvalidExecutionState,
    OpenObligationsWithoutDecision,
    RevisionExhausted,
    DigestMismatch(&'static str),
    DuplicateId(ProtocolId),
    MissingReference { kind: &'static str, id: ProtocolId },
    DependencyCycle(ProtocolId),
    EmptyRequiredCollection(&'static str),
    UnexpectedNonEmptyCollection(&'static str),
    MissingField(&'static str),
    UnexpectedField(&'static str),
    StaleContextBinding,
    IncompleteProofReuse,
    FrontierNotAntichain,
    ClosedObligationInFrontier(ProtocolId),
    ClosedObligationInDecision(ProtocolId),
    InvalidBudget,
    ClosurePartitionMismatch,
    DecisionObligationSetMismatch,
    FinalizedWithOpenObligations,
    RunMismatch,
    RevisionMismatch,
    ProposalMismatch,
    InvalidRouteProgram(&'static str),
    ActionAdmissionMismatch,
    ExecutionGrantMismatch,
    InvalidExecutionGrant,
    RecommendedNextOutsideSearch,
    InvalidRecommendedNext,
    DuplicateActionIdentity,
    RecommendedNextDoesNotAdvance,
    ParserRejectedCommand(String),
    ClosureNotFinalized,
    ClosureReceiptMismatch,
    ClosureProofMismatch(ProtocolId),
    IncompleteClosureScope(ProtocolId),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProtocolId(value) => write!(formatter, "invalid protocol id `{value}`"),
            Self::InvalidDigest(value) => write!(formatter, "invalid digest `{value}`"),
            Self::InvalidSchemaId(value) => write!(formatter, "invalid schema id `{value}`"),
            Self::InvalidSchemaVersion(value) => {
                write!(formatter, "invalid schema version `{value}`")
            }
            Self::InvalidCanonicalizationProfile(value) => {
                write!(formatter, "invalid canonicalization profile `{value}`")
            }
            Self::UnsafeInteger(field) => {
                write!(formatter, "{field} exceeds the JSON safe-integer profile")
            }
            Self::StateChainMismatch => formatter.write_str("state chain mismatch"),
            Self::InvalidActiveProgram => formatter.write_str("invalid active program binding"),
            Self::InvalidExecutionState => formatter.write_str("invalid execution state"),
            Self::OpenObligationsWithoutDecision => {
                formatter.write_str("open obligations require a decision")
            }
            Self::RevisionExhausted => formatter.write_str("state revision exhausted"),
            Self::DigestMismatch(field) => write!(formatter, "digest mismatch for {field}"),
            Self::DuplicateId(id) => write!(formatter, "duplicate id `{id}`"),
            Self::MissingReference { kind, id } => {
                write!(formatter, "missing {kind} reference `{id}`")
            }
            Self::DependencyCycle(id) => {
                write!(formatter, "obligation dependency cycle at `{id}`")
            }
            Self::EmptyRequiredCollection(field) => {
                write!(formatter, "{field} must be non-empty")
            }
            Self::UnexpectedNonEmptyCollection(field) => {
                write!(formatter, "{field} must be empty")
            }
            Self::MissingField(field) => write!(formatter, "missing required field {field}"),
            Self::UnexpectedField(field) => write!(formatter, "unexpected field {field}"),
            Self::StaleContextBinding => formatter.write_str("stale context binding"),
            Self::IncompleteProofReuse => formatter.write_str("complete proof reuse is incomplete"),
            Self::FrontierNotAntichain => formatter.write_str("frontier is not an antichain"),
            Self::ClosedObligationInFrontier(id) => {
                write!(formatter, "closed obligation `{id}` appears in frontier")
            }
            Self::ClosedObligationInDecision(id) => {
                write!(formatter, "closed obligation `{id}` appears in decision")
            }
            Self::InvalidBudget => formatter.write_str("invalid zero search budget"),
            Self::ClosurePartitionMismatch => formatter.write_str("closure partition mismatch"),
            Self::DecisionObligationSetMismatch => {
                formatter.write_str("search decision obligation set mismatch")
            }
            Self::FinalizedWithOpenObligations => {
                formatter.write_str("closure finalized with open obligations")
            }
            Self::RunMismatch => formatter.write_str("run identity mismatch"),
            Self::RevisionMismatch => formatter.write_str("state revision mismatch"),
            Self::ProposalMismatch => formatter.write_str("route proposal mismatch"),
            Self::InvalidRouteProgram(reason) => {
                write!(formatter, "invalid route program: {reason}")
            }
            Self::ActionAdmissionMismatch => formatter.write_str("action admission mismatch"),
            Self::ExecutionGrantMismatch => formatter.write_str("execution grant mismatch"),
            Self::InvalidExecutionGrant => formatter.write_str("invalid execution grant"),
            Self::RecommendedNextOutsideSearch => {
                formatter.write_str("recommendedNext outside search decision")
            }
            Self::InvalidRecommendedNext => formatter.write_str("invalid recommendedNext argv"),
            Self::DuplicateActionIdentity => formatter.write_str("duplicate action identity"),
            Self::RecommendedNextDoesNotAdvance => {
                formatter.write_str("recommendedNext does not advance an open obligation")
            }
            Self::ParserRejectedCommand(reason) => {
                write!(formatter, "parser rejected recommendedNext: {reason}")
            }
            Self::ClosureNotFinalized => formatter.write_str("closure is not finalized"),
            Self::ClosureReceiptMismatch => formatter.write_str("closure receipt mismatch"),
            Self::ClosureProofMismatch(id) => {
                write!(formatter, "closure proof mismatch for `{id}`")
            }
            Self::IncompleteClosureScope(id) => {
                write!(formatter, "incomplete closure scope for `{id}`")
            }
        }
    }
}

impl std::error::Error for ValidationError {}
