use std::path::PathBuf;
use std::time::Instant;

use agent_semantic_client_db::{
    ClientDbSourceIndexRefreshRequest, runtime_server_workspace::WorkspaceCanonicalMaterialization,
};

pub(super) struct PreparedSourceIndexGeneration {
    refresh_request: ClientDbSourceIndexRefreshRequest,
    materialization: WorkspaceCanonicalMaterialization,
}

impl PreparedSourceIndexGeneration {
    pub(super) fn new(
        _db_path: PathBuf,
        refresh_request: ClientDbSourceIndexRefreshRequest,
        materialization: WorkspaceCanonicalMaterialization,
        _trace_started: Instant,
    ) -> Self {
        Self {
            refresh_request,
            materialization,
        }
    }

    pub(super) fn into_runtime_server_build(
        self,
    ) -> agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuild {
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuild::new(
            self.refresh_request,
            self.materialization,
        )
    }
}
