//! Turso Merkle artifact graph adapter.

use std::path::Path;

use crate::types::{
    ClientDbArtifactEdge, ClientDbArtifactHash, ClientDbArtifactRepairChainFrame,
    ClientDbArtifactRoot, ClientDbProofReceipt,
};

use super::turso::connect_turso_client_db;
use super::turso_statement::{
    execute_turso_operation, execute_turso_statement, run_turso_operation,
};

async fn bootstrap_turso_artifact_graph_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_artifact_root (
            repo_id TEXT NOT NULL,
            workspace_id TEXT NOT NULL,
            scope_id TEXT NOT NULL,
            generation TEXT NOT NULL,
            root_kind TEXT NOT NULL,
            root_hash_algorithm TEXT NOT NULL,
            root_hash TEXT NOT NULL,
            node_hash_algorithm TEXT NOT NULL,
            node_hash TEXT NOT NULL,
            producer_hash_algorithm TEXT,
            producer_hash TEXT,
            schema_hash_algorithm TEXT,
            schema_hash TEXT,
            content_hash_algorithm TEXT,
            content_hash TEXT,
            PRIMARY KEY(repo_id, workspace_id, scope_id, generation, root_kind, root_hash)
        )",
        "CREATE INDEX IF NOT EXISTS asp_artifact_root_hash_idx
            ON asp_artifact_root(root_hash, root_kind)",
        "CREATE TABLE IF NOT EXISTS asp_artifact_edge (
            edge_hash_algorithm TEXT NOT NULL,
            edge_hash TEXT NOT NULL PRIMARY KEY,
            role TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            parent_root_hash TEXT NOT NULL,
            parent_root_kind TEXT NOT NULL,
            child_root_hash TEXT NOT NULL,
            child_root_kind TEXT NOT NULL,
            parent_json TEXT NOT NULL,
            child_json TEXT NOT NULL
        )",
        "CREATE INDEX IF NOT EXISTS asp_artifact_edge_parent_idx
            ON asp_artifact_edge(parent_root_hash, role, ordinal)",
        "CREATE INDEX IF NOT EXISTS asp_artifact_edge_child_idx
            ON asp_artifact_edge(child_root_hash, role, ordinal)",
        "CREATE TABLE IF NOT EXISTS asp_artifact_repair_chain_frame (
            root_hash TEXT NOT NULL PRIMARY KEY,
            frame_kind TEXT NOT NULL,
            root_json TEXT NOT NULL,
            content_hash_algorithm TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            parents_json TEXT NOT NULL
        )",
        "CREATE INDEX IF NOT EXISTS asp_artifact_repair_chain_frame_kind_idx
            ON asp_artifact_repair_chain_frame(frame_kind)",
        "CREATE TABLE IF NOT EXISTS asp_proof_receipt (
            receipt_id TEXT NOT NULL PRIMARY KEY,
            obligation_id TEXT NOT NULL,
            recipe_id TEXT NOT NULL,
            checker TEXT NOT NULL,
            environment TEXT NOT NULL,
            okay INTEGER NOT NULL,
            trust_level TEXT NOT NULL,
            summary_for_agent TEXT NOT NULL,
            root_hash TEXT NOT NULL,
            root_json TEXT NOT NULL
        )",
        "CREATE INDEX IF NOT EXISTS asp_proof_receipt_root_idx
            ON asp_proof_receipt(root_hash, okay)",
    ] {
        execute_turso_statement(
            connection,
            statement,
            "failed to bootstrap Turso artifact graph schema",
        )
        .await?;
    }
    Ok(())
}

pub async fn upsert_turso_artifact_roots(
    db_path: &Path,
    roots: &[ClientDbArtifactRoot],
) -> Result<u32, String> {
    if roots.is_empty() {
        return Ok(0);
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_artifact_graph_schema(&connection).await?;
    for root in roots {
        upsert_turso_artifact_root_with_connection(&connection, root).await?;
    }
    Ok(u32::try_from(roots.len()).unwrap_or(u32::MAX))
}

pub async fn upsert_turso_artifact_edges(
    db_path: &Path,
    edges: &[ClientDbArtifactEdge],
) -> Result<u32, String> {
    if edges.is_empty() {
        return Ok(0);
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_artifact_graph_schema(&connection).await?;
    for edge in edges {
        upsert_turso_artifact_root_with_connection(&connection, &edge.parent).await?;
        upsert_turso_artifact_root_with_connection(&connection, &edge.child).await?;
        let parent_json = serde_json::to_string(&edge.parent)
            .map_err(|error| format!("failed to encode Turso artifact edge parent: {error}"))?;
        let child_json = serde_json::to_string(&edge.child)
            .map_err(|error| format!("failed to encode Turso artifact edge child: {error}"))?;
        execute_turso_operation(
            || async {
                connection
                    .execute(
                        "INSERT INTO asp_artifact_edge (
                            edge_hash_algorithm,
                            edge_hash,
                            role,
                            ordinal,
                            parent_root_hash,
                            parent_root_kind,
                            child_root_hash,
                            child_root_kind,
                            parent_json,
                            child_json
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                        ON CONFLICT(edge_hash) DO UPDATE SET
                            role = excluded.role,
                            ordinal = excluded.ordinal,
                            parent_root_hash = excluded.parent_root_hash,
                            parent_root_kind = excluded.parent_root_kind,
                            child_root_hash = excluded.child_root_hash,
                            child_root_kind = excluded.child_root_kind,
                            parent_json = excluded.parent_json,
                            child_json = excluded.child_json",
                        (
                            edge.edge_hash.algorithm.as_str(),
                            edge.edge_hash.value.as_str(),
                            edge.role.as_str(),
                            i64::from(edge.ordinal),
                            edge.parent.root_hash.value.as_str(),
                            edge.parent.root_kind.as_str(),
                            edge.child.root_hash.value.as_str(),
                            edge.child.root_kind.as_str(),
                            parent_json.as_str(),
                            child_json.as_str(),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to upsert Turso artifact edge",
        )
        .await?;
    }
    Ok(u32::try_from(edges.len()).unwrap_or(u32::MAX))
}

pub async fn upsert_turso_repair_chain_frames(
    db_path: &Path,
    frames: &[ClientDbArtifactRepairChainFrame],
) -> Result<u32, String> {
    if frames.is_empty() {
        return Ok(0);
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_artifact_graph_schema(&connection).await?;
    for frame in frames {
        upsert_turso_artifact_root_with_connection(&connection, &frame.root).await?;
        let root_json = serde_json::to_string(&frame.root)
            .map_err(|error| format!("failed to encode Turso repair-chain root: {error}"))?;
        let parents_json = serde_json::to_string(&frame.parents)
            .map_err(|error| format!("failed to encode Turso repair-chain parents: {error}"))?;
        execute_turso_operation(
            || async {
                connection
                    .execute(
                        "INSERT INTO asp_artifact_repair_chain_frame (
                            root_hash,
                            frame_kind,
                            root_json,
                            content_hash_algorithm,
                            content_hash,
                            parents_json
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                        ON CONFLICT(root_hash) DO UPDATE SET
                            frame_kind = excluded.frame_kind,
                            root_json = excluded.root_json,
                            content_hash_algorithm = excluded.content_hash_algorithm,
                            content_hash = excluded.content_hash,
                            parents_json = excluded.parents_json",
                        (
                            frame.root.root_hash.value.as_str(),
                            frame.frame_kind.as_str(),
                            root_json.as_str(),
                            frame.content_hash.algorithm.as_str(),
                            frame.content_hash.value.as_str(),
                            parents_json.as_str(),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to upsert Turso repair-chain frame",
        )
        .await?;
    }
    Ok(u32::try_from(frames.len()).unwrap_or(u32::MAX))
}

pub async fn upsert_turso_proof_receipts(
    db_path: &Path,
    receipts: &[ClientDbProofReceipt],
) -> Result<u32, String> {
    if receipts.is_empty() {
        return Ok(0);
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_artifact_graph_schema(&connection).await?;
    for receipt in receipts {
        upsert_turso_artifact_root_with_connection(&connection, &receipt.root).await?;
        let root_json = serde_json::to_string(&receipt.root)
            .map_err(|error| format!("failed to encode Turso proof receipt root: {error}"))?;
        execute_turso_operation(
            || async {
                connection
                    .execute(
                        "INSERT INTO asp_proof_receipt (
                            receipt_id,
                            obligation_id,
                            recipe_id,
                            checker,
                            environment,
                            okay,
                            trust_level,
                            summary_for_agent,
                            root_hash,
                            root_json
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                        ON CONFLICT(receipt_id) DO UPDATE SET
                            obligation_id = excluded.obligation_id,
                            recipe_id = excluded.recipe_id,
                            checker = excluded.checker,
                            environment = excluded.environment,
                            okay = excluded.okay,
                            trust_level = excluded.trust_level,
                            summary_for_agent = excluded.summary_for_agent,
                            root_hash = excluded.root_hash,
                            root_json = excluded.root_json",
                        (
                            receipt.receipt_id.as_str(),
                            receipt.obligation_id.as_str(),
                            receipt.recipe_id.as_str(),
                            receipt.checker.as_str(),
                            receipt.environment.as_str(),
                            if receipt.okay { 1_i64 } else { 0_i64 },
                            receipt.trust_level.as_str(),
                            receipt.summary_for_agent.as_str(),
                            receipt.root.root_hash.value.as_str(),
                            root_json.as_str(),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to upsert Turso proof receipt",
        )
        .await?;
    }
    Ok(u32::try_from(receipts.len()).unwrap_or(u32::MAX))
}

pub async fn lookup_turso_artifact_edges(
    db_path: &Path,
    parent_root_hash: Option<&str>,
    limit: u32,
) -> Result<Vec<ClientDbArtifactEdge>, String> {
    if limit == 0 || !db_path.exists() {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_artifact_graph_schema(&connection).await?;
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT edge_hash_algorithm,
                    edge_hash,
                    role,
                    ordinal,
                    parent_json,
                    child_json
             FROM asp_artifact_edge
             WHERE (?1 IS NULL OR parent_root_hash = ?1)
             ORDER BY parent_root_hash ASC, ordinal ASC, role ASC
             LIMIT ?2",
                    (parent_root_hash, i64::from(limit)),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso artifact edges",
    )
    .await?;
    let mut edges = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso artifact edge row: {error}"))?
    {
        edges.push(turso_artifact_edge_from_row(&row)?);
    }
    Ok(edges)
}

pub async fn lookup_turso_repair_chain_frames(
    db_path: &Path,
    frame_kind: Option<&str>,
    limit: u32,
) -> Result<Vec<ClientDbArtifactRepairChainFrame>, String> {
    if limit == 0 || !db_path.exists() {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_artifact_graph_schema(&connection).await?;
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT frame_kind, root_json, content_hash_algorithm, content_hash, parents_json
             FROM asp_artifact_repair_chain_frame
             WHERE (?1 IS NULL OR frame_kind = ?1)
             ORDER BY root_hash ASC
             LIMIT ?2",
                    (frame_kind, i64::from(limit)),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso repair-chain frames",
    )
    .await?;
    let mut frames = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso repair-chain frame row: {error}"))?
    {
        frames.push(turso_repair_chain_frame_from_row(&row)?);
    }
    Ok(frames)
}

pub async fn lookup_turso_proof_receipts(
    db_path: &Path,
    root_hash: Option<&str>,
    limit: u32,
) -> Result<Vec<ClientDbProofReceipt>, String> {
    if limit == 0 || !db_path.exists() {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    bootstrap_turso_artifact_graph_schema(&connection).await?;
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT receipt_id,
                    obligation_id,
                    recipe_id,
                    checker,
                    environment,
                    okay,
                    trust_level,
                    summary_for_agent,
                    root_json
             FROM asp_proof_receipt
             WHERE (?1 IS NULL OR root_hash = ?1)
             ORDER BY receipt_id ASC
             LIMIT ?2",
                    (root_hash, i64::from(limit)),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso proof receipts",
    )
    .await?;
    let mut receipts = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso proof receipt row: {error}"))?
    {
        receipts.push(turso_proof_receipt_from_row(&row)?);
    }
    Ok(receipts)
}

async fn upsert_turso_artifact_root_with_connection(
    connection: &turso::Connection,
    root: &ClientDbArtifactRoot,
) -> Result<(), String> {
    execute_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_artifact_root (
                        repo_id,
                        workspace_id,
                        scope_id,
                        generation,
                        root_kind,
                        root_hash_algorithm,
                        root_hash,
                        node_hash_algorithm,
                        node_hash,
                        producer_hash_algorithm,
                        producer_hash,
                        schema_hash_algorithm,
                        schema_hash,
                        content_hash_algorithm,
                        content_hash
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                    ON CONFLICT(repo_id, workspace_id, scope_id, generation, root_kind, root_hash)
                    DO UPDATE SET
                        node_hash_algorithm = excluded.node_hash_algorithm,
                        node_hash = excluded.node_hash,
                        producer_hash_algorithm = excluded.producer_hash_algorithm,
                        producer_hash = excluded.producer_hash,
                        schema_hash_algorithm = excluded.schema_hash_algorithm,
                        schema_hash = excluded.schema_hash,
                        content_hash_algorithm = excluded.content_hash_algorithm,
                        content_hash = excluded.content_hash",
                    (
                        root.repo_id.as_str(),
                        root.workspace_id.as_str(),
                        root.scope_id.as_str(),
                        root.generation.as_str(),
                        root.root_kind.as_str(),
                        root.root_hash.algorithm.as_str(),
                        root.root_hash.value.as_str(),
                        root.node_hash.algorithm.as_str(),
                        root.node_hash.value.as_str(),
                        root.producer_hash
                            .as_ref()
                            .map(|hash| hash.algorithm.as_str()),
                        root.producer_hash.as_ref().map(|hash| hash.value.as_str()),
                        root.schema_hash
                            .as_ref()
                            .map(|hash| hash.algorithm.as_str()),
                        root.schema_hash.as_ref().map(|hash| hash.value.as_str()),
                        root.content_hash
                            .as_ref()
                            .map(|hash| hash.algorithm.as_str()),
                        root.content_hash.as_ref().map(|hash| hash.value.as_str()),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to upsert Turso artifact root",
    )
    .await?;
    Ok(())
}

fn turso_artifact_edge_from_row(row: &turso::Row) -> Result<ClientDbArtifactEdge, String> {
    let ordinal = row
        .get::<i64>(3)
        .map_err(|error| format!("failed to read Turso artifact edge ordinal: {error}"))?
        .max(0)
        .min(i64::from(u32::MAX)) as u32;
    let parent_json = row
        .get::<String>(4)
        .map_err(|error| format!("failed to read Turso artifact edge parent JSON: {error}"))?;
    let child_json = row
        .get::<String>(5)
        .map_err(|error| format!("failed to read Turso artifact edge child JSON: {error}"))?;
    Ok(ClientDbArtifactEdge {
        edge_hash: ClientDbArtifactHash {
            algorithm: row.get::<String>(0).map_err(|error| {
                format!("failed to read Turso artifact edge hash algorithm: {error}")
            })?,
            value: row
                .get::<String>(1)
                .map_err(|error| format!("failed to read Turso artifact edge hash: {error}"))?,
        },
        role: row
            .get::<String>(2)
            .map_err(|error| format!("failed to read Turso artifact edge role: {error}"))?,
        ordinal,
        parent: serde_json::from_str(&parent_json).map_err(|error| {
            format!("failed to decode Turso artifact edge parent root: {error}")
        })?,
        child: serde_json::from_str(&child_json)
            .map_err(|error| format!("failed to decode Turso artifact edge child root: {error}"))?,
    })
}

fn turso_repair_chain_frame_from_row(
    row: &turso::Row,
) -> Result<ClientDbArtifactRepairChainFrame, String> {
    let root_json = row
        .get::<String>(1)
        .map_err(|error| format!("failed to read Turso repair-chain root JSON: {error}"))?;
    let parents_json = row
        .get::<String>(4)
        .map_err(|error| format!("failed to read Turso repair-chain parents JSON: {error}"))?;
    Ok(ClientDbArtifactRepairChainFrame {
        frame_kind: row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso repair-chain frame kind: {error}"))?,
        root: serde_json::from_str(&root_json)
            .map_err(|error| format!("failed to decode Turso repair-chain root: {error}"))?,
        content_hash: ClientDbArtifactHash {
            algorithm: row.get::<String>(2).map_err(|error| {
                format!("failed to read Turso repair-chain content hash algorithm: {error}")
            })?,
            value: row.get::<String>(3).map_err(|error| {
                format!("failed to read Turso repair-chain content hash: {error}")
            })?,
        },
        parents: serde_json::from_str(&parents_json)
            .map_err(|error| format!("failed to decode Turso repair-chain parents: {error}"))?,
    })
}

fn turso_proof_receipt_from_row(row: &turso::Row) -> Result<ClientDbProofReceipt, String> {
    let root_json = row
        .get::<String>(8)
        .map_err(|error| format!("failed to read Turso proof receipt root JSON: {error}"))?;
    Ok(ClientDbProofReceipt {
        receipt_id: row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso proof receipt id: {error}"))?,
        obligation_id: row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso proof obligation id: {error}"))?,
        recipe_id: row
            .get::<String>(2)
            .map_err(|error| format!("failed to read Turso proof recipe id: {error}"))?,
        checker: row
            .get::<String>(3)
            .map_err(|error| format!("failed to read Turso proof checker: {error}"))?,
        environment: row
            .get::<String>(4)
            .map_err(|error| format!("failed to read Turso proof environment: {error}"))?,
        okay: row
            .get::<i64>(5)
            .map_err(|error| format!("failed to read Turso proof okay flag: {error}"))?
            != 0,
        trust_level: row
            .get::<String>(6)
            .map_err(|error| format!("failed to read Turso proof trust level: {error}"))?,
        summary_for_agent: row
            .get::<String>(7)
            .map_err(|error| format!("failed to read Turso proof summary: {error}"))?,
        root: serde_json::from_str(&root_json)
            .map_err(|error| format!("failed to decode Turso proof receipt root: {error}"))?,
    })
}
