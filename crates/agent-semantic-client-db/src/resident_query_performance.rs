use serde::{Deserialize, Serialize};

pub const RESIDENT_QUERY_PERFORMANCE_SCHEMA_ID: &str =
    "asp.resident-query-performance-receipt";
pub const RESIDENT_QUERY_PERFORMANCE_SCHEMA_VERSION: &str = "1";
pub const RESIDENT_QUERY_BUDGET_MICROS: u64 = 1_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ResidentWorkspaceReference {
    CheckoutRoot {
        #[serde(rename = "workspaceRoot")]
        workspace_root: String,
    },
    WorkspaceId {
        #[serde(rename = "workspaceId")]
        workspace_id: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResidentQueryPhase {
    ColdSelectorCacheMiss,
    WarmExactQuery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidentQueryConcurrency {
    pub workspace_count: u32,
    pub session_count: u32,
    pub request_count: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidentQueryIoCounters {
    pub fs_reads: u64,
    pub db_opens: u64,
    pub manifest_reads: u64,
    pub process_spawns: u64,
    pub lock_probes: u64,
    pub schema_bootstraps: u64,
    pub workspace_canonicalizations: u64,
}

impl ResidentQueryIoCounters {
    pub const ZERO: Self = Self {
        fs_reads: 0,
        db_opens: 0,
        manifest_reads: 0,
        process_spawns: 0,
        lock_probes: 0,
        schema_bootstraps: 0,
        workspace_canonicalizations: 0,
    };

    fn is_zero(self) -> bool {
        self == Self::ZERO
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidentQueryPerformanceReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace: ResidentWorkspaceReference,
    pub generation_root: String,
    pub root_depth: [u8; 2],
    pub phase: ResidentQueryPhase,
    pub elapsed_micros: u64,
    pub budget_micros: u64,
    pub concurrency: ResidentQueryConcurrency,
    pub io: ResidentQueryIoCounters,
}

impl ResidentQueryPerformanceReceipt {
    pub fn zero_io(
        workspace: ResidentWorkspaceReference,
        generation_root: String,
        phase: ResidentQueryPhase,
        elapsed_micros: u64,
        concurrency: ResidentQueryConcurrency,
    ) -> Result<Self, String> {
        let receipt = Self {
            schema_id: RESIDENT_QUERY_PERFORMANCE_SCHEMA_ID.to_owned(),
            schema_version: RESIDENT_QUERY_PERFORMANCE_SCHEMA_VERSION.to_owned(),
            workspace,
            generation_root,
            root_depth: [1, 0],
            phase,
            elapsed_micros,
            budget_micros: RESIDENT_QUERY_BUDGET_MICROS,
            concurrency,
            io: ResidentQueryIoCounters::ZERO,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RESIDENT_QUERY_PERFORMANCE_SCHEMA_ID
            || self.schema_version != RESIDENT_QUERY_PERFORMANCE_SCHEMA_VERSION
        {
            return Err("resident query performance schema identity mismatch".to_owned());
        }
        if self.root_depth != [1, 0] {
            return Err("resident query performance rootDepth must be [1,0]".to_owned());
        }
        let digest = self
            .generation_root
            .strip_prefix("blake3-256:")
            .filter(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            });
        if digest.is_none() {
            return Err("resident query generationRoot must be a blake3-256 digest".to_owned());
        }
        if self.budget_micros != RESIDENT_QUERY_BUDGET_MICROS
            || self.elapsed_micros > self.budget_micros
        {
            return Err(format!(
                "resident query exceeded {}us budget: elapsedMicros={}",
                RESIDENT_QUERY_BUDGET_MICROS, self.elapsed_micros
            ));
        }
        if self.concurrency.workspace_count == 0
            || self.concurrency.session_count == 0
            || self.concurrency.request_count == 0
        {
            return Err("resident query concurrency counts must be positive".to_owned());
        }
        if !self.io.is_zero() {
            return Err("resident query data plane performed forbidden I/O".to_owned());
        }
        Ok(())
    }
}
