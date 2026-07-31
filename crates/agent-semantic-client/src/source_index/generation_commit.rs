use std::path::PathBuf;
use std::time::Instant;

use agent_semantic_client_db::{
    ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest,
    runtime_server_workspace::WorkspaceCanonicalMaterialization,
};

use super::model::SourceIndexRefreshReport;

pub(super) struct PreparedSourceIndexGeneration {
    db_path: PathBuf,
    refresh_request: ClientDbSourceIndexRefreshRequest,
    materialization: WorkspaceCanonicalMaterialization,
    trace_started: Instant,
}

impl PreparedSourceIndexGeneration {
    pub(super) fn new(
        db_path: PathBuf,
        refresh_request: ClientDbSourceIndexRefreshRequest,
        materialization: WorkspaceCanonicalMaterialization,
        trace_started: Instant,
    ) -> Self {
        Self {
            db_path,
            refresh_request,
            materialization,
            trace_started,
        }
    }

    pub(super) fn commit(self) -> Result<SourceIndexRefreshReport, String> {
        let Self {
            db_path,
            refresh_request,
            materialization,
            trace_started,
        } = self;
        let report =
            agent_semantic_client_db::workspace_db_ipc::
                commit_source_index_generation_via_runtime_server(
                    refresh_request,
                    materialization,
                )?;
        Ok(Self::finish_report(db_path, trace_started, report))
    }

    pub(super) async fn commit_async(self) -> Result<SourceIndexRefreshReport, String> {
        let Self {
            db_path,
            refresh_request,
            materialization,
            trace_started,
        } = self;
        let project_root = refresh_request.import.project_root.clone();
        let session =
            agent_semantic_client_db::workspace_db_ipc::connect_runtime_server_workspace_session(
                &project_root,
            )
            .await?;
        let report = session
            .commit_source_index_generation(&refresh_request, materialization)
            .await?;
        Ok(Self::finish_report(db_path, trace_started, report))
    }

    fn finish_report(
        db_path: PathBuf,
        trace_started: Instant,
        report: ClientDbSourceIndexRefreshReport,
    ) -> SourceIndexRefreshReport {
        if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
            eprintln!(
                "[source-index-trace] stage=generation-turso-imported elapsedMs={}",
                trace_started.elapsed().as_millis()
            );
        }
        SourceIndexRefreshReport::from_report(db_path, report.clone(), report.source_snapshot)
    }
}
