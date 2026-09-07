// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub(super) fn build_fixture_source_index_import(
    mut request: agent_semantic_client_db::ClientDbSourceIndexImportRequest,
) -> Result<agent_semantic_client_db::ClientDbSourceIndexImport, String> {
    request.source_blobs =
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
            request.files.iter().map(|file| {
                (
                    agent_semantic_client_db::ClientDbSourceIndexPath::from(
                        file.relative_path.clone(),
                    ),
                    file.text.as_bytes().to_vec(),
                )
            }),
        );
    agent_semantic_client_db::build_source_index_import(request)
}
