use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::Path,
    sync::Arc,
    time::UNIX_EPOCH,
};

use opentelemetry::{KeyValue, Value, trace::Status};
use opentelemetry_sdk::{
    error::{OTelSdkError, OTelSdkResult},
    trace::{SpanData, SpanExporter},
};
use tokio::sync::Mutex;

use super::semconv;

const PERFORMANCE_SPAN_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-opentelemetry-performance-event";
const PERFORMANCE_SPAN_SCHEMA_VERSION: &str = "1";

fn optional_u64(row: &turso::Row, index: usize, label: &str) -> Result<Option<u64>, String> {
    let value = row
        .get::<Option<i64>>(index)
        .map_err(|error| format!("failed to decode {label}: {error}"))?;
    value
        .map(|value| u64::try_from(value).map_err(|_| format!("{label} is negative: {value}")))
        .transpose()
}

#[derive(Clone)]
pub struct TursoOpenTelemetrySpanExporter {
    _database: Arc<turso::Database>,
    writer: Arc<Mutex<turso::Connection>>,
    reader: Arc<Mutex<turso::Connection>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveSearchIncident {
    pub incident_id: String,
    pub workspace_identity: String,
    pub language_id: Option<String>,
    pub generation_digest: Option<String>,
    pub canonical_request_digest: Option<String>,
    pub reason_kind: Option<String>,
    pub incident_state: String,
    pub incident_transition: String,
    pub requested_projection: Option<String>,
    pub observed_at_unix_micros: u64,
    pub elapsed_micros: Option<u64>,
    pub budget_micros: Option<u64>,
}

impl fmt::Debug for TursoOpenTelemetrySpanExporter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TursoOpenTelemetrySpanExporter")
            .finish_non_exhaustive()
    }
}

impl TursoOpenTelemetrySpanExporter {
    pub async fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                format!(
                    "failed to create Runtime Server OpenTelemetry directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let database = crate::engine::shared_turso_database(path).await?;
        let writer = database.connect().map_err(|error| {
            format!("failed to connect Runtime Server OpenTelemetry Turso DB: {error}")
        })?;
        bootstrap_schema(&writer).await?;
        let reader = database.connect().map_err(|error| {
            format!("failed to connect Runtime Server OpenTelemetry Turso reader: {error}")
        })?;
        Ok(Self {
            _database: database,
            writer: Arc::new(Mutex::new(writer)),
            reader: Arc::new(Mutex::new(reader)),
        })
    }

    pub async fn budget_failure_count(&self, surface: &str, stage: &str) -> Result<u64, String> {
        let connection = self.reader.lock().await;
        let mut rows = connection
            .query(
                "SELECT COUNT(*) FROM asp_otel_performance_span
                 WHERE surface = ?1 AND stage = ?2 AND budget_status = 'budget-exceeded'",
                (surface, stage),
            )
            .await
            .map_err(|error| format!("failed to query OpenTelemetry budget failures: {error}"))?;
        let row = rows
            .next()
            .await
            .map_err(|error| format!("failed to read OpenTelemetry budget count: {error}"))?
            .ok_or_else(|| "OpenTelemetry budget count returned no row".to_owned())?;
        let count = row
            .get::<i64>(0)
            .map_err(|error| format!("failed to decode OpenTelemetry budget count: {error}"))?;
        u64::try_from(count).map_err(|_| format!("OpenTelemetry budget count is negative: {count}"))
    }

    pub async fn runtime_pressure_count_for_workspace(
        &self,
        workspace_identity: &str,
    ) -> Result<u64, String> {
        let connection = self.reader.lock().await;
        let mut rows = connection
            .query(
                "SELECT COUNT(*) FROM asp_otel_runtime_pressure
                 WHERE workspace_identity = ?1",
                [workspace_identity],
            )
            .await
            .map_err(|error| format!("failed to query OpenTelemetry runtime pressure: {error}"))?;
        let row = rows
            .next()
            .await
            .map_err(|error| {
                format!("failed to read OpenTelemetry runtime pressure count: {error}")
            })?
            .ok_or_else(|| "OpenTelemetry runtime pressure count returned no row".to_owned())?;
        let count = row.get::<i64>(0).map_err(|error| {
            format!("failed to decode OpenTelemetry runtime pressure count: {error}")
        })?;
        u64::try_from(count)
            .map_err(|_| format!("OpenTelemetry runtime pressure count is negative: {count}"))
    }

    pub async fn budget_failure_count_for_workspace(
        &self,
        workspace_identity: &str,
        surface: &str,
        stage: &str,
    ) -> Result<u64, String> {
        let connection = self.reader.lock().await;
        let mut rows = connection
            .query(
                "SELECT COUNT(*) FROM asp_otel_performance_span
                 WHERE workspace_identity = ?1 AND surface = ?2 AND stage = ?3
                   AND budget_status = 'budget-exceeded'",
                (workspace_identity, surface, stage),
            )
            .await
            .map_err(|error| {
                format!("failed to query workspace OpenTelemetry budget failures: {error}")
            })?;
        let row = rows
            .next()
            .await
            .map_err(|error| format!("failed to read workspace budget count: {error}"))?
            .ok_or_else(|| "workspace OpenTelemetry budget count returned no row".to_owned())?;
        let count = row.get::<i64>(0).map_err(|error| {
            format!("failed to decode workspace OpenTelemetry budget count: {error}")
        })?;
        u64::try_from(count)
            .map_err(|_| format!("workspace OpenTelemetry budget count is negative: {count}"))
    }

    pub async fn latest_attributes_json_for_workspace(
        &self,
        workspace_identity: &str,
        surface: &str,
        stage: &str,
    ) -> Result<Option<String>, String> {
        let connection = self.reader.lock().await;
        let mut rows = connection
            .query(
                "SELECT attributes_json FROM asp_otel_performance_span
                 WHERE workspace_identity = ?1 AND surface = ?2 AND stage = ?3
                 ORDER BY start_time_unix_nanos DESC LIMIT 1",
                (workspace_identity, surface, stage),
            )
            .await
            .map_err(|error| {
                format!("failed to query latest workspace OpenTelemetry attributes: {error}")
            })?;
        let Some(row) = rows.next().await.map_err(|error| {
            format!("failed to read latest workspace OpenTelemetry attributes: {error}")
        })?
        else {
            return Ok(None);
        };
        row.get::<String>(0)
            .map(Some)
            .map_err(|error| format!("failed to decode OpenTelemetry attributes JSON: {error}"))
    }

    pub async fn active_search_incidents(
        &self,
        workspace_identity: &str,
        limit: u64,
    ) -> Result<Vec<ActiveSearchIncident>, String> {
        let connection = self.reader.lock().await;
        let limit = i64::try_from(limit.max(1)).unwrap_or(i64::MAX);
        let mut rows = connection
            .query(
                "SELECT incident_id, workspace_identity, language_id, generation_digest,
                        operation_id, failure_reason, incident_state, incident_transition,
                        requested_projection, observed_at_unix_micros, elapsed_micros, budget_micros
                 FROM asp_otel_active_search_incident
                 WHERE workspace_identity = ?1
                 ORDER BY observed_at_unix_micros DESC
                 LIMIT ?2",
                (workspace_identity, limit),
            )
            .await
            .map_err(|error| format!("failed to query active search incidents: {error}"))?;
        let mut incidents = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|error| format!("failed to read active search incident: {error}"))?
        {
            let observed_at = row
                .get::<i64>(9)
                .map_err(|error| format!("failed to decode incident observation time: {error}"))?;
            incidents.push(ActiveSearchIncident {
                incident_id: row
                    .get(0)
                    .map_err(|error| format!("failed to decode incident id: {error}"))?,
                workspace_identity: row
                    .get(1)
                    .map_err(|error| format!("failed to decode incident workspace: {error}"))?,
                language_id: row
                    .get(2)
                    .map_err(|error| format!("failed to decode incident language: {error}"))?,
                generation_digest: row
                    .get(3)
                    .map_err(|error| format!("failed to decode incident generation: {error}"))?,
                canonical_request_digest: row
                    .get(4)
                    .map_err(|error| format!("failed to decode incident request: {error}"))?,
                reason_kind: row
                    .get(5)
                    .map_err(|error| format!("failed to decode incident reason: {error}"))?,
                incident_state: row
                    .get(6)
                    .map_err(|error| format!("failed to decode incident state: {error}"))?,
                incident_transition: row
                    .get(7)
                    .map_err(|error| format!("failed to decode incident transition: {error}"))?,
                requested_projection: row
                    .get(8)
                    .map_err(|error| format!("failed to decode requested projection: {error}"))?,
                observed_at_unix_micros: u64::try_from(observed_at)
                    .map_err(|_| format!("incident observation time is negative: {observed_at}"))?,
                elapsed_micros: optional_u64(&row, 10, "incident elapsed time")?,
                budget_micros: optional_u64(&row, 11, "incident budget")?,
            });
        }
        Ok(incidents)
    }

    pub async fn budget_failure_count_for_event_identity(
        &self,
        event_identity: &str,
    ) -> Result<u64, String> {
        let connection = self.reader.lock().await;
        let mut rows = connection
            .query(
                "SELECT COUNT(*) FROM asp_otel_performance_span
                 WHERE event_identity = ?1 AND budget_status = 'budget-exceeded'",
                (event_identity,),
            )
            .await
            .map_err(|error| format!("failed to query OpenTelemetry event identity: {error}"))?;
        let row = rows
            .next()
            .await
            .map_err(|error| format!("failed to read event identity count: {error}"))?
            .ok_or_else(|| "OpenTelemetry event identity count returned no row".to_owned())?;
        let count = row.get::<i64>(0).map_err(|error| {
            format!("failed to decode OpenTelemetry event identity count: {error}")
        })?;
        u64::try_from(count)
            .map_err(|_| format!("OpenTelemetry event identity count is negative: {count}"))
    }

    async fn export_batch(&self, batch: &[SpanData]) -> Result<(), String> {
        if batch.is_empty() {
            return Ok(());
        }
        let connection = self.writer.lock().await;
        let transaction = connection
            .unchecked_transaction()
            .await
            .map_err(|error| format!("failed to begin OpenTelemetry Turso transaction: {error}"))?;
        for span in batch {
            let attributes = span_attributes(&span.attributes);
            let surface = string_attribute(&attributes, semconv::SURFACE);
            let stage = string_attribute(&attributes, semconv::STAGE);
            let workspace_identity =
                optional_string_attribute(&attributes, semconv::WORKSPACE_IDENTITY);
            let language_id = optional_string_attribute(&attributes, semconv::LANGUAGE_ID);
            let generation_digest =
                optional_string_attribute(&attributes, semconv::GENERATION_DIGEST);
            let runtime_artifact_digest =
                optional_string_attribute(&attributes, semconv::RUNTIME_ARTIFACT_DIGEST);
            let transport_contract_digest =
                optional_string_attribute(&attributes, semconv::TRANSPORT_CONTRACT_DIGEST);
            let failure_reason = optional_string_attribute(&attributes, semconv::FAILURE_REASON);
            let retry_after_ms = optional_integer_attribute(&attributes, semconv::RETRY_AFTER_MS);
            let event_identity = optional_string_attribute(&attributes, semconv::EVENT_IDENTITY);
            let observed_at_unix_micros =
                optional_integer_attribute(&attributes, semconv::OBSERVED_AT_UNIX_MICROS);
            let budget_status = string_attribute(&attributes, semconv::BUDGET_STATUS);
            let elapsed_micros = integer_attribute(&attributes, semconv::ELAPSED_MICROS);
            let budget_micros = integer_attribute(&attributes, semconv::BUDGET_MICROS);
            let process_resident_bytes =
                optional_integer_attribute(&attributes, semconv::PROCESS_RESIDENT_MEMORY);
            let process_peak_resident_bytes =
                optional_integer_attribute(&attributes, semconv::PROCESS_PEAK_RESIDENT_MEMORY);
            let process_memory_budget_bytes =
                optional_integer_attribute(&attributes, semconv::PROCESS_MEMORY_BUDGET);
            let process_memory_budget_status =
                optional_string_attribute(&attributes, semconv::PROCESS_MEMORY_BUDGET_STATUS);
            let runtime_event_loop_lag_micros =
                optional_integer_attribute(&attributes, semconv::RUNTIME_EVENT_LOOP_LAG);
            let runtime_alive_tasks =
                optional_integer_attribute(&attributes, semconv::RUNTIME_ALIVE_TASKS);
            let runtime_global_queue_depth =
                optional_integer_attribute(&attributes, semconv::RUNTIME_GLOBAL_QUEUE_DEPTH);
            let runtime_active_connections =
                optional_integer_attribute(&attributes, semconv::RUNTIME_ACTIVE_CONNECTIONS);
            let runtime_connection_limit =
                optional_integer_attribute(&attributes, semconv::RUNTIME_CONNECTION_LIMIT);
            let runtime_connection_high_watermark =
                optional_integer_attribute(&attributes, semconv::RUNTIME_CONNECTION_HIGH_WATERMARK);
            let runtime_rejected_connections =
                optional_integer_attribute(&attributes, semconv::RUNTIME_REJECTED_CONNECTIONS);
            let runtime_diagnostic_queue_depth =
                optional_integer_attribute(&attributes, semconv::RUNTIME_DIAGNOSTIC_QUEUE_DEPTH);
            let runtime_diagnostic_queue_capacity =
                optional_integer_attribute(&attributes, semconv::RUNTIME_DIAGNOSTIC_QUEUE_CAPACITY);
            let runtime_dropped_diagnostics =
                optional_integer_attribute(&attributes, semconv::RUNTIME_DROPPED_DIAGNOSTICS);
            let attributes_json = serde_json::to_string(&attributes)
                .map_err(|error| format!("failed to encode OpenTelemetry attributes: {error}"))?;
            let (status_code, status_description) = status_projection(&span.status);
            transaction
                .execute(
                    "INSERT INTO asp_otel_performance_span (
                        schema_id, schema_version,
                        trace_id, span_id, parent_span_id, span_name,
                        start_time_unix_nanos, end_time_unix_nanos,
                        status_code, status_description, surface, stage,
                        workspace_identity, language_id, generation_digest,
                        runtime_artifact_digest, transport_contract_digest,
                        failure_reason, retry_after_ms,
                        event_identity, observed_at_unix_micros,
                        elapsed_micros, budget_micros, budget_status, attributes_json
                    ) VALUES (
                        'agent.semantic-protocols.runtime-server-opentelemetry-performance-event',
                        '1',
                        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                        ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                        ?18, ?19, ?20, ?21, ?22, ?23
                    )
                    ON CONFLICT DO NOTHING",
                    turso::params![
                        span.span_context.trace_id().to_string(),
                        span.span_context.span_id().to_string(),
                        span.parent_span_id.to_string(),
                        span.name.as_ref(),
                        unix_nanos(span.start_time)?,
                        unix_nanos(span.end_time)?,
                        status_code,
                        status_description,
                        surface,
                        stage,
                        workspace_identity.as_deref(),
                        language_id,
                        generation_digest,
                        runtime_artifact_digest,
                        transport_contract_digest,
                        failure_reason,
                        retry_after_ms,
                        event_identity,
                        observed_at_unix_micros,
                        elapsed_micros,
                        budget_micros,
                        budget_status,
                        attributes_json,
                    ],
                )
                .await
                .map_err(|error| format!("failed to persist OpenTelemetry span: {error}"))?;
            let pressure_present = [
                process_resident_bytes,
                process_peak_resident_bytes,
                process_memory_budget_bytes,
                runtime_event_loop_lag_micros,
                runtime_alive_tasks,
                runtime_global_queue_depth,
                runtime_active_connections,
                runtime_connection_limit,
                runtime_connection_high_watermark,
                runtime_rejected_connections,
                runtime_diagnostic_queue_depth,
                runtime_diagnostic_queue_capacity,
                runtime_dropped_diagnostics,
            ]
            .into_iter()
            .any(|value| value.is_some());
            if pressure_present {
                transaction
                    .execute(
                        "INSERT INTO asp_otel_runtime_pressure (
                            trace_id, span_id, observed_at_unix_nanos, surface, stage,
                            workspace_identity,
                            process_resident_bytes, process_peak_resident_bytes,
                            process_memory_budget_bytes, process_memory_budget_status,
                            runtime_event_loop_lag_micros, runtime_alive_tasks,
                            runtime_global_queue_depth, runtime_active_connections,
                            runtime_connection_limit, runtime_connection_high_watermark,
                            runtime_rejected_connections, runtime_diagnostic_queue_depth,
                            runtime_diagnostic_queue_capacity, runtime_dropped_diagnostics
                        ) VALUES (
                            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20
                        ) ON CONFLICT(trace_id, span_id) DO NOTHING",
                        turso::params![
                            span.span_context.trace_id().to_string(),
                            span.span_context.span_id().to_string(),
                            unix_nanos(span.start_time)?,
                            string_attribute(&attributes, semconv::SURFACE),
                            string_attribute(&attributes, semconv::STAGE),
                            workspace_identity.as_deref(),
                            process_resident_bytes,
                            process_peak_resident_bytes,
                            process_memory_budget_bytes,
                            process_memory_budget_status,
                            runtime_event_loop_lag_micros,
                            runtime_alive_tasks,
                            runtime_global_queue_depth,
                            runtime_active_connections,
                            runtime_connection_limit,
                            runtime_connection_high_watermark,
                            runtime_rejected_connections,
                            runtime_diagnostic_queue_depth,
                            runtime_diagnostic_queue_capacity,
                            runtime_dropped_diagnostics,
                        ],
                    )
                    .await
                    .map_err(|error| {
                        format!("failed to persist OpenTelemetry runtime pressure: {error}")
                    })?;
            }
        }
        transaction
            .commit()
            .await
            .map_err(|error| format!("failed to commit OpenTelemetry span batch: {error}"))
    }
}

impl SpanExporter for TursoOpenTelemetrySpanExporter {
    fn export(&self, batch: Vec<SpanData>) -> impl Future<Output = OTelSdkResult> + Send {
        async move {
            self.export_batch(&batch)
                .await
                .map_err(OTelSdkError::InternalFailure)
        }
    }
}

async fn bootstrap_schema(connection: &turso::Connection) -> Result<(), String> {
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS asp_otel_performance_span (
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            trace_id TEXT NOT NULL,
            span_id TEXT NOT NULL,
            parent_span_id TEXT NOT NULL,
            span_name TEXT NOT NULL,
            start_time_unix_nanos INTEGER NOT NULL,
            end_time_unix_nanos INTEGER NOT NULL,
            status_code TEXT NOT NULL,
            status_description TEXT,
            surface TEXT,
            stage TEXT,
            workspace_identity TEXT,
            language_id TEXT,
            generation_digest TEXT,
            runtime_artifact_digest TEXT,
            transport_contract_digest TEXT,
            failure_reason TEXT,
            retry_after_ms INTEGER,
            event_identity TEXT,
            observed_at_unix_micros INTEGER,
            elapsed_micros INTEGER,
            budget_micros INTEGER,
            budget_status TEXT,
            attributes_json TEXT NOT NULL,
            PRIMARY KEY (trace_id, span_id)
        )",
            (),
        )
        .await
        .map_err(|error| format!("failed to create Runtime Server telemetry table: {error}"))?;
    ensure_performance_span_columns(connection).await?;
    connection
        .execute(
            "CREATE INDEX IF NOT EXISTS asp_otel_search_incident_timeline_idx
             ON asp_otel_performance_span(
                 json_extract(attributes_json, '$.\"asp.search.incident.id\"'),
                 observed_at_unix_micros DESC
             )
             WHERE json_extract(attributes_json, '$.\"asp.search.incident.id\"') IS NOT NULL",
            (),
        )
        .await
        .map_err(|error| format!("failed to create search incident timeline index: {error}"))?;
    connection
        .execute(
            "CREATE VIEW IF NOT EXISTS asp_otel_active_search_incident AS
             WITH ranked AS (
                 SELECT
                     workspace_identity,
                     language_id,
                     generation_digest,
                     json_extract(attributes_json, '$.\"asp.operation.id\"') AS operation_id,
                     failure_reason,
                     observed_at_unix_micros,
                     elapsed_micros,
                     budget_micros,
                     attributes_json,
                     json_extract(attributes_json, '$.\"asp.search.incident.id\"') AS incident_id,
                     json_extract(attributes_json, '$.\"asp.search.incident.state\"') AS incident_state,
                     json_extract(attributes_json, '$.\"asp.search.incident.transition\"') AS incident_transition,
                     json_extract(attributes_json, '$.\"asp.search.requested_projection\"') AS requested_projection,
                     ROW_NUMBER() OVER (
                         PARTITION BY workspace_identity,
                             json_extract(attributes_json, '$.\"asp.search.incident.id\"')
                         ORDER BY observed_at_unix_micros DESC,
                             json_extract(attributes_json, '$.\"asp.search.incident.transition_sequence\"') DESC,
                             start_time_unix_nanos DESC
                     ) AS incident_rank
                 FROM asp_otel_performance_span
                 WHERE json_extract(attributes_json, '$.\"asp.search.incident.id\"') IS NOT NULL
             )
             SELECT * FROM ranked
             WHERE incident_rank = 1
               AND incident_state IN (
                   'open', 'repairing', 'verification-pending', 'failed-verification'
               )",
            (),
        )
        .await
        .map_err(|error| format!("failed to create active search incident view: {error}"))?;
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS asp_otel_runtime_pressure (
                trace_id TEXT NOT NULL,
                span_id TEXT NOT NULL,
                observed_at_unix_nanos INTEGER NOT NULL,
                surface TEXT NOT NULL,
                stage TEXT NOT NULL,
                workspace_identity TEXT,
                process_resident_bytes INTEGER,
                process_peak_resident_bytes INTEGER,
                process_memory_budget_bytes INTEGER,
                process_memory_budget_status TEXT,
                runtime_event_loop_lag_micros INTEGER,
                runtime_alive_tasks INTEGER,
                runtime_global_queue_depth INTEGER,
                runtime_active_connections INTEGER,
                runtime_connection_limit INTEGER,
                runtime_connection_high_watermark INTEGER,
                runtime_rejected_connections INTEGER,
                runtime_diagnostic_queue_depth INTEGER,
                runtime_diagnostic_queue_capacity INTEGER,
                runtime_dropped_diagnostics INTEGER,
                PRIMARY KEY (trace_id, span_id)
            )",
            (),
        )
        .await
        .map_err(|error| format!("failed to create Runtime pressure telemetry table: {error}"))?;
    ensure_runtime_pressure_columns(connection).await?;
    for statement in [
        "CREATE INDEX IF NOT EXISTS asp_otel_performance_stage_idx
            ON asp_otel_performance_span(surface, stage, budget_status, start_time_unix_nanos)",
        "CREATE INDEX IF NOT EXISTS asp_otel_performance_workspace_stage_idx
            ON asp_otel_performance_span(
                workspace_identity, surface, stage, budget_status, start_time_unix_nanos
            )",
        "CREATE INDEX IF NOT EXISTS asp_otel_performance_timeline_idx
            ON asp_otel_performance_span(start_time_unix_nanos, trace_id, span_id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS asp_otel_performance_event_identity_idx
            ON asp_otel_performance_span(event_identity)
            WHERE event_identity IS NOT NULL",
        "CREATE INDEX IF NOT EXISTS asp_otel_runtime_pressure_timeline_idx
            ON asp_otel_runtime_pressure(observed_at_unix_nanos, surface, stage)",
        "CREATE INDEX IF NOT EXISTS asp_otel_runtime_pressure_workspace_timeline_idx
            ON asp_otel_runtime_pressure(workspace_identity, observed_at_unix_nanos)",
    ] {
        connection.execute(statement, ()).await.map_err(|error| {
            format!("failed to bootstrap Runtime Server OpenTelemetry schema: {error}")
        })?;
    }
    Ok(())
}

async fn ensure_runtime_pressure_columns(connection: &turso::Connection) -> Result<(), String> {
    let mut rows = connection
        .query("PRAGMA table_info(asp_otel_runtime_pressure)", ())
        .await
        .map_err(|error| format!("failed to inspect Runtime pressure telemetry schema: {error}"))?;
    let mut columns = BTreeSet::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Runtime pressure telemetry schema: {error}"))?
    {
        columns.insert(row.get::<String>(1).map_err(|error| {
            format!("failed to decode Runtime pressure telemetry column name: {error}")
        })?);
    }
    if !columns.contains("workspace_identity") {
        connection
            .execute(
                "ALTER TABLE asp_otel_runtime_pressure ADD COLUMN workspace_identity TEXT",
                (),
            )
            .await
            .map_err(|error| {
                format!("failed to add Runtime pressure telemetry workspace identity: {error}")
            })?;
    }
    Ok(())
}

async fn ensure_performance_span_columns(connection: &turso::Connection) -> Result<(), String> {
    let mut rows = connection
        .query("PRAGMA table_info(asp_otel_performance_span)", ())
        .await
        .map_err(|error| format!("failed to inspect Runtime Server telemetry schema: {error}"))?;
    let mut columns = BTreeSet::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Runtime Server telemetry schema: {error}"))?
    {
        columns.insert(
            row.get::<String>(1)
                .map_err(|error| format!("failed to decode telemetry column name: {error}"))?,
        );
    }
    for (column, statement) in [
        (
            "schema_id",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN schema_id TEXT NOT NULL DEFAULT 'agent.semantic-protocols.runtime-server-opentelemetry-performance-event'",
        ),
        (
            "schema_version",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN schema_version TEXT NOT NULL DEFAULT '1'",
        ),
        (
            "workspace_identity",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN workspace_identity TEXT",
        ),
        (
            "language_id",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN language_id TEXT",
        ),
        (
            "generation_digest",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN generation_digest TEXT",
        ),
        (
            "runtime_artifact_digest",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN runtime_artifact_digest TEXT",
        ),
        (
            "transport_contract_digest",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN transport_contract_digest TEXT",
        ),
        (
            "failure_reason",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN failure_reason TEXT",
        ),
        (
            "retry_after_ms",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN retry_after_ms INTEGER",
        ),
        (
            "event_identity",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN event_identity TEXT",
        ),
        (
            "observed_at_unix_micros",
            "ALTER TABLE asp_otel_performance_span ADD COLUMN observed_at_unix_micros INTEGER",
        ),
    ] {
        if !columns.contains(column) {
            connection.execute(statement, ()).await.map_err(|error| {
                format!("failed to add Runtime Server telemetry column {column}: {error}")
            })?;
        }
    }
    Ok(())
}

fn span_attributes(attributes: &[KeyValue]) -> BTreeMap<String, serde_json::Value> {
    attributes
        .iter()
        .map(|attribute| {
            (
                attribute.key.as_str().to_owned(),
                match &attribute.value {
                    Value::Bool(value) => serde_json::Value::Bool(*value),
                    Value::I64(value) => serde_json::Value::from(*value),
                    Value::F64(value) => serde_json::Value::from(*value),
                    Value::String(value) => serde_json::Value::String(value.as_str().to_owned()),
                    Value::Array(value) => serde_json::Value::String(value.to_string()),
                    _ => serde_json::Value::String(attribute.value.to_string()),
                },
            )
        })
        .chain([
            (
                "asp.schema.id".to_owned(),
                serde_json::Value::String(PERFORMANCE_SPAN_SCHEMA_ID.to_owned()),
            ),
            (
                "asp.schema.version".to_owned(),
                serde_json::Value::String(PERFORMANCE_SPAN_SCHEMA_VERSION.to_owned()),
            ),
        ])
        .collect()
}

fn string_attribute(attributes: &BTreeMap<String, serde_json::Value>, key: &str) -> String {
    attributes
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn integer_attribute(attributes: &BTreeMap<String, serde_json::Value>, key: &str) -> i64 {
    attributes
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default()
}

fn optional_string_attribute(
    attributes: &BTreeMap<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    attributes
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn optional_integer_attribute(
    attributes: &BTreeMap<String, serde_json::Value>,
    key: &str,
) -> Option<i64> {
    attributes.get(key).and_then(serde_json::Value::as_i64)
}

fn status_projection(status: &Status) -> (&'static str, Option<String>) {
    match status {
        Status::Unset => ("unset", None),
        Status::Ok => ("ok", None),
        Status::Error { description } => ("error", Some(description.to_string())),
    }
}

fn unix_nanos(time: std::time::SystemTime) -> Result<i64, String> {
    let nanos = time
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("OpenTelemetry span time predates Unix epoch: {error}"))?
        .as_nanos();
    i64::try_from(nanos)
        .map_err(|_| format!("OpenTelemetry span timestamp exceeds Turso INTEGER range: {nanos}"))
}
