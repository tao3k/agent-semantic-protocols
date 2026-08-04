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

#[derive(Clone)]
pub struct TursoOpenTelemetrySpanExporter {
    _database: Arc<turso::Database>,
    writer: Arc<Mutex<turso::Connection>>,
    reader: Arc<Mutex<turso::Connection>>,
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
            let budget_status = string_attribute(&attributes, semconv::BUDGET_STATUS);
            let elapsed_micros = integer_attribute(&attributes, semconv::ELAPSED_MICROS);
            let budget_micros = integer_attribute(&attributes, semconv::BUDGET_MICROS);
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
                        elapsed_micros, budget_micros, budget_status, attributes_json
                    ) VALUES (
                        'agent.semantic-protocols.runtime-server-opentelemetry-performance-event',
                        '1',
                        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                        ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                        ?18, ?19, ?20, ?21
                    )
                    ON CONFLICT(trace_id, span_id) DO NOTHING",
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
                        workspace_identity,
                        language_id,
                        generation_digest,
                        runtime_artifact_digest,
                        transport_contract_digest,
                        failure_reason,
                        retry_after_ms,
                        elapsed_micros,
                        budget_micros,
                        budget_status,
                        attributes_json,
                    ],
                )
                .await
                .map_err(|error| format!("failed to persist OpenTelemetry span: {error}"))?;
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
    for statement in [
        "CREATE INDEX IF NOT EXISTS asp_otel_performance_stage_idx
            ON asp_otel_performance_span(surface, stage, budget_status, start_time_unix_nanos)",
        "CREATE INDEX IF NOT EXISTS asp_otel_performance_workspace_stage_idx
            ON asp_otel_performance_span(
                workspace_identity, surface, stage, budget_status, start_time_unix_nanos
            )",
        "CREATE INDEX IF NOT EXISTS asp_otel_performance_timeline_idx
            ON asp_otel_performance_span(start_time_unix_nanos, trace_id, span_id)",
    ] {
        connection.execute(statement, ()).await.map_err(|error| {
            format!("failed to bootstrap Runtime Server OpenTelemetry schema: {error}")
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
