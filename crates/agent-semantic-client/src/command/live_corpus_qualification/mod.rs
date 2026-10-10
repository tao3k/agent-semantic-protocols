// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Live Corpus qualification command boundary.

pub(super) mod client_protocol;
mod contract;
pub(crate) mod protocol_model;
mod query_protocol;
mod runner;
#[path = "runner_contract.rs"]
mod runner_contract;
mod runner_model;
mod runner_prepare;
mod search_receipt;
mod workspace_fixture;

pub(in crate::command::live_corpus) use contract::AgentOrgTopologyEvidence;
pub(in crate::command::live_corpus) use query_protocol::public_query;
pub(in crate::command::live_corpus) use query_protocol::source_query_scheme_template;
pub(super) use runner::run;
pub(super) use runner_contract::validate_args;
pub(super) use workspace_fixture::IsolatedBenchmarkWorkspace;
