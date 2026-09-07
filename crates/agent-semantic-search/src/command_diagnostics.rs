// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use serde::Deserialize;
use serde::Serialize;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Instant;

pub const SEARCH_COMMAND_DIAGNOSTICS_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-command-diagnostics";
pub const SEARCH_COMMAND_DIAGNOSTICS_SCHEMA_VERSION: &str = "1";

static NEXT_TRACE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchCommandKind {
    Search,
    Query,
}

impl SearchCommandKind {
    fn from_args(args: &[String]) -> Option<Self> {
        match args.first().map(String::as_str) {
            Some("search") => Some(Self::Search),
            Some("query") => Some(Self::Query),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchCommandDiagnosticOptions {
    pub verbose: bool,
    pub debug: bool,
    pub trace: bool,
}

impl SearchCommandDiagnosticOptions {
    pub fn is_enabled(self) -> bool {
        self.verbose || self.debug || self.trace
    }

    fn projections(self) -> Vec<SearchCommandDiagnosticProjection> {
        let mut values = Vec::with_capacity(3);
        if self.verbose {
            values.push(SearchCommandDiagnosticProjection::Verbose);
        }
        if self.debug {
            values.push(SearchCommandDiagnosticProjection::Debug);
        }
        if self.trace {
            values.push(SearchCommandDiagnosticProjection::Trace);
        }
        values
    }
}

pub fn take_search_command_diagnostic_options(
    args: &mut Vec<String>,
) -> Result<SearchCommandDiagnosticOptions, String> {
    if SearchCommandKind::from_args(args).is_none() {
        return Ok(SearchCommandDiagnosticOptions::default());
    }
    let mut options = SearchCommandDiagnosticOptions::default();
    let mut retained = Vec::with_capacity(args.len());
    let mut provider_data = false;
    for argument in args.drain(..) {
        if provider_data {
            retained.push(argument);
            continue;
        }
        if argument == "--" {
            provider_data = true;
            retained.push(argument);
            continue;
        }
        let target = match argument.as_str() {
            "--verbose" => Some((&mut options.verbose, "--verbose")),
            "--debug" => Some((&mut options.debug, "--debug")),
            "--trace" => Some((&mut options.trace, "--trace")),
            _ => None,
        };
        if let Some((enabled, flag)) = target {
            if *enabled {
                return Err(format!("diagnostics flag `{flag}` must not be repeated"));
            }
            *enabled = true;
        } else {
            retained.push(argument);
        }
    }
    *args = retained;
    Ok(options)
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCommandResourceCounters {
    pub database_opens: u64,
    pub binary_reads: u64,
    pub manifest_writes: u64,
    pub provider_process_starts: u64,
    pub ipc_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCommandTraceStage {
    pub sequence: u64,
    pub stage: String,
    pub elapsed_micros: u64,
    pub total_micros: u64,
    pub counters: SearchCommandResourceCounters,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCommandDebugProjection {
    pub generation_digest: Option<String>,
    pub provider_id: Option<String>,
    pub route_kind: Option<String>,
    pub cache_state: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SearchCommandDiagnosticProjection {
    Verbose,
    Debug,
    Trace,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCommandDiagnosticsReceipt {
    schema_id: &'static str,
    schema_version: &'static str,
    pub trace_id: String,
    pub command_identity: String,
    pub command_kind: SearchCommandKind,
    pub language_id: String,
    pub workspace_identity: Option<String>,
    projections: Vec<SearchCommandDiagnosticProjection>,
    pub total_elapsed_micros: u64,
    pub budget_micros: u64,
    pub budget_status: String,
    pub bug: bool,
    pub failure_reason: Option<String>,
    pub debug: Option<SearchCommandDebugProjection>,
    pub stages: Vec<SearchCommandTraceStage>,
}

pub struct SearchCommandDiagnostics {
    options: SearchCommandDiagnosticOptions,
    language_id: String,
    command_kind: SearchCommandKind,
    normalized_args: Vec<String>,
    workspace_identity: Option<String>,
    command_identity: String,
    trace_id: String,
    started: Instant,
    stage_started: Instant,
    next_stage_sequence: u64,
    stages: Vec<SearchCommandTraceStage>,
    debug: SearchCommandDebugProjection,
}

impl SearchCommandDiagnostics {
    pub fn start(
        language_id: &str,
        normalized_args: &[String],
        options: SearchCommandDiagnosticOptions,
        started: Instant,
    ) -> Option<Self> {
        let command_kind = SearchCommandKind::from_args(normalized_args)?;
        let command_identity = command_identity(language_id, command_kind, None, normalized_args);
        let trace_id = blake3_identity(&(
            command_identity.as_str(),
            std::process::id(),
            NEXT_TRACE_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        Some(Self {
            options,
            language_id: language_id.to_owned(),
            command_kind,
            normalized_args: normalized_args.to_vec(),
            workspace_identity: None,
            command_identity,
            trace_id,
            started,
            stage_started: started,
            next_stage_sequence: 0,
            stages: Vec::new(),
            debug: SearchCommandDebugProjection::default(),
        })
    }

    pub fn bind_workspace_identity(&mut self, workspace_identity: impl Into<String>) {
        let workspace_identity = workspace_identity.into();
        self.command_identity = command_identity(
            &self.language_id,
            self.command_kind,
            Some(&workspace_identity),
            &self.normalized_args,
        );
        self.workspace_identity = Some(workspace_identity);
    }

    pub fn bind_debug_state(
        &mut self,
        generation_digest: Option<&str>,
        provider_id: Option<&str>,
        route_kind: Option<&str>,
        cache_state: Option<&str>,
    ) {
        self.debug = SearchCommandDebugProjection {
            generation_digest: generation_digest.map(str::to_owned),
            provider_id: provider_id.map(str::to_owned),
            route_kind: route_kind.map(str::to_owned),
            cache_state: cache_state.map(str::to_owned),
        };
    }

    pub fn mark_stage(&mut self, stage: impl Into<String>) {
        self.mark_stage_with_counters(stage, SearchCommandResourceCounters::default());
    }

    pub fn mark_stage_with_counters(
        &mut self,
        stage: impl Into<String>,
        counters: SearchCommandResourceCounters,
    ) {
        if !self.options.trace {
            return;
        }
        let now = Instant::now();
        self.stages.push(SearchCommandTraceStage {
            sequence: self.next_stage_sequence,
            stage: stage.into(),
            elapsed_micros: micros(now.duration_since(self.stage_started)),
            total_micros: micros(now.duration_since(self.started)),
            counters,
        });
        self.next_stage_sequence += 1;
        self.stage_started = now;
    }

    pub fn finish(
        mut self,
        budget_micros: u64,
        failure_reason: Option<&str>,
    ) -> SearchCommandDiagnosticsReceipt {
        if self.options.trace {
            self.mark_stage("command-complete");
        }
        let total_elapsed_micros = micros(Instant::now().duration_since(self.started));
        let budget_exceeded = total_elapsed_micros > budget_micros;
        SearchCommandDiagnosticsReceipt {
            schema_id: SEARCH_COMMAND_DIAGNOSTICS_SCHEMA_ID,
            schema_version: SEARCH_COMMAND_DIAGNOSTICS_SCHEMA_VERSION,
            trace_id: self.trace_id,
            command_identity: self.command_identity,
            command_kind: self.command_kind,
            language_id: self.language_id,
            workspace_identity: self.workspace_identity,
            projections: self.options.projections(),
            total_elapsed_micros,
            budget_micros,
            budget_status: if budget_exceeded {
                "budget-exceeded"
            } else {
                "within-budget"
            }
            .to_owned(),
            bug: budget_exceeded,
            failure_reason: failure_reason.map(str::to_owned),
            debug: self.options.debug.then_some(self.debug),
            stages: if self.options.trace {
                self.stages
            } else {
                Vec::new()
            },
        }
    }
}

fn command_identity(
    language_id: &str,
    command_kind: SearchCommandKind,
    workspace_identity: Option<&str>,
    normalized_args: &[String],
) -> String {
    blake3_identity(&(
        language_id,
        command_kind,
        workspace_identity,
        normalized_args,
    ))
}

fn blake3_identity(value: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(value).expect("diagnostics identity input must serialize");
    format!("blake3-256:{}", blake3::hash(&encoded).to_hex())
}

fn micros(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}
