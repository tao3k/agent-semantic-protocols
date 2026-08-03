use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use turso::{Builder, Database};

const TABLE: &str = "semantic_graph_turbo_cache_v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphTurboCacheIdentity {
    pub snapshot_digest: String,
    pub workspace_generation_root_digest: String,
    pub profile: String,
    pub algorithm: String,
    pub seed_digest: String,
    pub parameter_digest: String,
}

impl GraphTurboCacheIdentity {
    pub fn validate(&self) -> Result<(), String> {
        validate_digest("snapshotDigest", &self.snapshot_digest)?;
        validate_nonempty(
            "workspaceGenerationRootDigest",
            &self.workspace_generation_root_digest,
        )?;
        validate_nonempty("profile", &self.profile)?;
        validate_nonempty("algorithm", &self.algorithm)?;
        validate_digest("seedDigest", &self.seed_digest)?;
        validate_digest("parameterDigest", &self.parameter_digest)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphTurboProposalStatus {
    Candidate,
    Proposed,
    Heuristic,
    Partial,
}

impl GraphTurboProposalStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Proposed => "proposed",
            Self::Heuristic => "heuristic",
            Self::Partial => "partial",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "candidate" => Ok(Self::Candidate),
            "proposed" => Ok(Self::Proposed),
            "heuristic" => Ok(Self::Heuristic),
            "partial" => Ok(Self::Partial),
            other => Err(format!("unsupported Graph Turbo proposal status `{other}`")),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphTurboCacheEntry {
    pub identity: GraphTurboCacheIdentity,
    pub proposal_status: GraphTurboProposalStatus,
    pub result: Value,
}

impl GraphTurboCacheEntry {
    pub fn from_resident_receipt(
        identity: GraphTurboCacheIdentity,
        receipt: &Value,
    ) -> Result<Self, String> {
        identity.validate()?;
        if receipt.get("status").and_then(Value::as_str) != Some("rank-completed") {
            return Err("Graph Turbo receipt is not rank-completed".to_string());
        }
        if receipt.get("authority").and_then(Value::as_str) != Some("candidate") {
            return Err("Graph Turbo receipt authority must be candidate".to_string());
        }
        let result = receipt
            .get("result")
            .filter(|value| value.is_object())
            .cloned()
            .ok_or_else(|| "Graph Turbo receipt result must be an object".to_string())?;
        Ok(Self {
            identity,
            proposal_status: GraphTurboProposalStatus::Candidate,
            result,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphTurboCacheReadStatus {
    Hit,
    Miss,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphTurboCacheRead {
    pub status: GraphTurboCacheReadStatus,
    pub entry: Option<GraphTurboCacheEntry>,
}

pub struct TursoGraphTurboCache {
    database: Database,
}

impl TursoGraphTurboCache {
    pub async fn open(path: &Path) -> Result<Self, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "Graph Turbo Turso path is not valid UTF-8".to_string())?;
        let database = Builder::new_local(path)
            .build()
            .await
            .map_err(|error| format!("failed to open Graph Turbo Turso cache: {error}"))?;
        let cache = Self { database };
        cache.initialize().await?;
        Ok(cache)
    }

    pub async fn put(&self, entry: &GraphTurboCacheEntry) -> Result<(), String> {
        entry.identity.validate()?;
        let result_json = serde_json::to_string(&entry.result)
            .map_err(|error| format!("failed to encode Graph Turbo result: {error}"))?;
        self.connection()?
            .execute(
                &format!("INSERT INTO {TABLE} (snapshot_digest, workspace_generation_root_digest, profile, algorithm, seed_digest, parameter_digest, proposal_status, result_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(snapshot_digest, workspace_generation_root_digest, profile, algorithm, seed_digest, parameter_digest) DO UPDATE SET proposal_status = excluded.proposal_status, result_json = excluded.result_json"),
                [
                    entry.identity.snapshot_digest.as_str(),
                    entry.identity.workspace_generation_root_digest.as_str(),
                    entry.identity.profile.as_str(),
                    entry.identity.algorithm.as_str(),
                    entry.identity.seed_digest.as_str(),
                    entry.identity.parameter_digest.as_str(),
                    entry.proposal_status.as_str(),
                    result_json.as_str(),
                ],
            )
            .await
            .map_err(|error| format!("failed to write Graph Turbo Turso cache: {error}"))?;
        Ok(())
    }

    pub async fn get(
        &self,
        identity: &GraphTurboCacheIdentity,
    ) -> Result<GraphTurboCacheRead, String> {
        identity.validate()?;
        let mut rows = self.connection()?
            .query(
                &format!("SELECT proposal_status, result_json FROM {TABLE} WHERE snapshot_digest = ?1 AND workspace_generation_root_digest = ?2 AND profile = ?3 AND algorithm = ?4 AND seed_digest = ?5 AND parameter_digest = ?6"),
                [
                    identity.snapshot_digest.as_str(),
                    identity.workspace_generation_root_digest.as_str(),
                    identity.profile.as_str(),
                    identity.algorithm.as_str(),
                    identity.seed_digest.as_str(),
                    identity.parameter_digest.as_str(),
                ],
            )
            .await
            .map_err(|error| format!("failed to read Graph Turbo Turso cache: {error}"))?;
        let Some(row) = rows
            .next()
            .await
            .map_err(|error| format!("failed to advance Graph Turbo Turso row: {error}"))?
        else {
            return Ok(GraphTurboCacheRead {
                status: GraphTurboCacheReadStatus::Miss,
                entry: None,
            });
        };
        let status = text_column(&row, 0, "proposal status")?;
        let result_json = text_column(&row, 1, "result")?;
        Ok(GraphTurboCacheRead {
            status: GraphTurboCacheReadStatus::Hit,
            entry: Some(GraphTurboCacheEntry {
                identity: identity.clone(),
                proposal_status: GraphTurboProposalStatus::parse(&status)?,
                result: serde_json::from_str(&result_json)
                    .map_err(|error| format!("failed to parse Graph Turbo result: {error}"))?,
            }),
        })
    }

    fn connection(&self) -> Result<turso::Connection, String> {
        self.database
            .connect()
            .map_err(|error| format!("failed to connect to Graph Turbo Turso cache: {error}"))
    }

    async fn initialize(&self) -> Result<(), String> {
        self.connection()?
            .execute(
                &format!("CREATE TABLE IF NOT EXISTS {TABLE} (snapshot_digest TEXT NOT NULL, workspace_generation_root_digest TEXT NOT NULL, profile TEXT NOT NULL, algorithm TEXT NOT NULL, seed_digest TEXT NOT NULL, parameter_digest TEXT NOT NULL, proposal_status TEXT NOT NULL CHECK (proposal_status IN ('candidate', 'proposed', 'heuristic', 'partial')), result_json TEXT NOT NULL, PRIMARY KEY (snapshot_digest, workspace_generation_root_digest, profile, algorithm, seed_digest, parameter_digest))"),
                (),
            )
            .await
            .map_err(|error| format!("failed to initialize Graph Turbo Turso cache: {error}"))?;
        Ok(())
    }
}

fn text_column(row: &turso::Row, index: usize, field: &str) -> Result<String, String> {
    row.get_value(index)
        .map_err(|error| format!("failed to decode Graph Turbo {field}: {error}"))?
        .as_text()
        .cloned()
        .ok_or_else(|| format!("Graph Turbo {field} is not text"))
}

fn validate_digest(field: &str, value: &str) -> Result<(), String> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(format!(
            "{field} must be a 64-character lowercase hex digest"
        ))
    }
}

fn validate_nonempty(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}
