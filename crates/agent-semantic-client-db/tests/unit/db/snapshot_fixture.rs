// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::SourceSnapshotKind;

pub(crate) fn source_snapshot_evidence() -> SourceSnapshotEvidence {
    source_snapshot_evidence_for(1)
}

pub(crate) fn source_snapshot_evidence_for(revision: u64) -> SourceSnapshotEvidence {
    source_snapshot_evidence_for_files(revision, 1)
}

pub(crate) fn source_snapshot_evidence_for_files(
    revision: u64,
    leaf_count: usize,
) -> SourceSnapshotEvidence {
    SourceSnapshotEvidence::new(
        format!("{revision:064x}"),
        SourceSnapshotKind::Filesystem,
        leaf_count,
        format!("{:064x}", revision.saturating_add(1)),
    )
}
