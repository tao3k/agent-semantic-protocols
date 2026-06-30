use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{ClientDbBackend, ClientDbEngineDurability, ClientDbEngineFeatures};

/// Diagnostic report for the planned Turso DB Engine backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoClientDbEngineReport {
    pub backend: &'static str,
    pub status: &'static str,
    pub db_file_name: &'static str,
    pub schema_bootstrap: &'static str,
    pub durability: &'static str,
    pub features: ClientDbEngineFeatures,
    pub db_path: PathBuf,
    pub reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TursoClientDbEngineBackend;

impl TursoClientDbEngineBackend {
    pub(super) fn inspect(self, db_path: &Path) -> TursoClientDbEngineReport {
        TursoClientDbEngineReport {
            backend: ClientDbBackend::Turso.as_str(),
            status: "planned",
            db_file_name: "client.turso",
            schema_bootstrap: "pending-cutover",
            durability: ClientDbEngineDurability::TursoLocalFile.as_str(),
            features: ClientDbEngineFeatures {
                async_io: true,
                concurrent_writes: true,
                fts: true,
                vector: false,
                overlay_search: true,
                sync: true,
                encryption: false,
            },
            db_path: db_path.with_file_name("client.turso"),
            reason: Some("active backend remains sqlite-v1 until Turso cutover gates pass"),
        }
    }
}
