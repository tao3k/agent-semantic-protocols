// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::PathBuf;
use std::time::Instant;

use crate::{
    ClientDbSourceIndexRefreshRequest, runtime_server_workspace::WorkspaceCanonicalMaterialization,
};

pub(super) struct PreparedSourceIndexGeneration {
    refresh_request: ClientDbSourceIndexRefreshRequest,
    materialization: WorkspaceCanonicalMaterialization,
    candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
}

impl PreparedSourceIndexGeneration {
    pub(super) fn new(
        _db_path: PathBuf,
        refresh_request: ClientDbSourceIndexRefreshRequest,
        materialization: WorkspaceCanonicalMaterialization,
        candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
        _trace_started: Instant,
    ) -> Self {
        Self {
            refresh_request,
            materialization,
            candidate,
        }
    }

    pub(super) fn refresh_request(&self) -> &crate::ClientDbSourceIndexRefreshRequest {
        &self.refresh_request
    }

    pub(super) fn materialization(
        &self,
    ) -> &crate::runtime_server_workspace::WorkspaceCanonicalMaterialization {
        &self.materialization
    }

    pub(super) fn with_complete_successor(
        mut self,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
        file_count: u32,
        import: crate::ClientDbSourceIndexImport,
        materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    ) -> Self {
        self.refresh_request.source_snapshot = source_snapshot;
        self.refresh_request.file_count = file_count;
        self.refresh_request.import = import;
        self.materialization = materialization;
        self
    }

    pub(super) fn into_runtime_server_build(
        self,
    ) -> crate::runtime_server_admission::WorkspaceGenerationCandidateBuild {
        crate::runtime_server_admission::WorkspaceGenerationCandidateBuild::new(
            self.candidate,
            self.refresh_request,
            self.materialization,
        )
    }
}
