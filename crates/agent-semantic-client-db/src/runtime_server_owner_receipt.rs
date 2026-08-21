//! Runtime Server owner receipt, owned by client-db lifecycle composition.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerSpawnReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub process_id: u32,
    pub nonce: String,
    pub state_home: String,
    pub runtime_artifact_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerExitReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub owner_epoch: u64,
    pub clean_drain: bool,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerDrainReceipt {
    pub owner_epoch: u64,
    pub services: serde_json::Value,
    pub remaining_task_count: usize,
    pub remaining_child_count: usize,
    pub clean_drain: bool,
}
