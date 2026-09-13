// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

const QUERY_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-performance-query";
const QUERY_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-performance-query-receipt";
const SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimePerformanceQuery {
    schema_id: String,
    schema_version: String,
    pub workspace_identity: String,
    pub surface: String,
    pub stage: String,
}

impl RuntimePerformanceQuery {
    pub fn new(
        workspace_identity: impl Into<String>,
        surface: impl Into<String>,
        stage: impl Into<String>,
    ) -> Self {
        Self {
            schema_id: QUERY_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            workspace_identity: workspace_identity.into(),
            surface: surface.into(),
            stage: stage.into(),
        }
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if self.schema_id != QUERY_SCHEMA_ID
            || self.schema_version != SCHEMA_VERSION
            || self.workspace_identity.is_empty()
            || self.surface.is_empty()
            || self.stage.is_empty()
        {
            return Err("Runtime Server performance query is incomplete".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimePerformanceQueryReceipt {
    schema_id: String,
    schema_version: String,
    pub workspace_identity: String,
    pub surface: String,
    pub stage: String,
    pub observation_count: u64,
    pub budget_failure_count: u64,
    pub p50_micros: Option<u64>,
    pub p95_micros: Option<u64>,
    pub p99_micros: Option<u64>,
    pub latest_attributes_json: Option<String>,
}

impl RuntimePerformanceQueryReceipt {
    pub(super) fn new(
        query: RuntimePerformanceQuery,
        summary: super::live_store::LivePerformanceSummary,
    ) -> Self {
        Self {
            schema_id: QUERY_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            workspace_identity: query.workspace_identity,
            surface: query.surface,
            stage: query.stage,
            observation_count: summary.observation_count,
            budget_failure_count: summary.budget_failure_count,
            p50_micros: summary.p50_micros,
            p95_micros: summary.p95_micros,
            p99_micros: summary.p99_micros,
            latest_attributes_json: summary.latest_attributes_json,
        }
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if self.schema_id != QUERY_RECEIPT_SCHEMA_ID
            || self.schema_version != SCHEMA_VERSION
            || self.workspace_identity.is_empty()
            || self.surface.is_empty()
            || self.stage.is_empty()
        {
            return Err("Runtime Server performance query receipt is incomplete".to_owned());
        }
        Ok(())
    }
}
