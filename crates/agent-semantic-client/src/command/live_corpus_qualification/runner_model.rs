// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Admitted Live Corpus execution plan values shared by preparation and running.

use std::path::PathBuf;

use super::contract::QualificationCase;
use super::protocol_model::ResidentSearchLatencyBudget;
use crate::command::live_corpus::LiveCorpusQualification;

pub(super) const DEFAULT_PLAN_PATH: &str = "benchmarks/live-corpus-scheme-scenarios.v1.toml";
pub(super) const DEFAULT_TOPOLOGY_PLAN_PATH: &str =
    "benchmarks/live-corpus-agent-org-topology-scenarios.v1.toml";

#[derive(Debug)]
pub(super) struct QualifyArgs {
    pub(super) plan_path: PathBuf,
    pub(super) resource_id: Option<String>,
    pub(super) language_id: Option<String>,
    pub(super) json: bool,
}

pub(super) struct PreparedCase {
    pub(super) case: QualificationCase,
    pub(super) agent_prompt: String,
    pub(super) required_relation_kinds: Vec<String>,
    pub(super) composed_search: String,
    pub(super) multi_source_query: String,
    pub(super) multi_callable_skeleton_query: String,
    pub(super) minimum_composed_candidates: usize,
    pub(super) checkout_path: PathBuf,
    pub(super) remote: String,
    pub(super) qualification: LiveCorpusQualification,
    pub(super) artifact_digest: String,
}

pub(super) struct PreparedRun {
    pub(super) args: QualifyArgs,
    pub(super) plan_bytes: Vec<u8>,
    pub(super) lock_bytes: Vec<u8>,
    pub(super) runtime_state_home: PathBuf,
    pub(super) resource_state_home: PathBuf,
    pub(super) resident_sample_count: usize,
    pub(super) sequential_sample_count: usize,
    pub(super) concurrent_sample_count: usize,
    pub(super) cold_load_sample_count: usize,
    pub(super) protocol_qualified_case_count: usize,
    pub(super) resident_search_latency_budget: ResidentSearchLatencyBudget,
    pub(super) cases: Vec<PreparedCase>,
}
