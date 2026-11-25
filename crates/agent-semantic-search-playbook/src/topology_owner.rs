// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Validation for generation-bound Topology owner membership predicates.

use crate::{ProgressiveSearchPlaybookError, TopologyOwnerMembershipBlock};

pub(crate) fn validate_block(
    block: TopologyOwnerMembershipBlock,
) -> Result<TopologyOwnerMembershipBlock, ProgressiveSearchPlaybookError> {
    if block.kind != "file"
        || [
            &block.exact_path,
            &block.path_prefix,
            &block.extension,
            &block.path_glob,
        ]
        .into_iter()
        .all(|value| value.is_none())
    {
        return Err(ProgressiveSearchPlaybookError::IncompleteRequest(
            "topology owners requires (kind file) and at least one path, path-prefix, extension, or path-glob predicate".to_owned(),
        ));
    }
    if block.extension.as_deref().is_some_and(|extension| {
        extension.is_empty()
            || !extension
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'+' | b'-'))
    }) || block
        .exact_path
        .iter()
        .chain(block.path_prefix.iter())
        .chain(block.path_glob.iter())
        .any(|path| invalid_path_filter(path))
    {
        return Err(ProgressiveSearchPlaybookError::InvalidParameter {
            reason_kind: "search-playbook-topology-filter-invalid",
            message: "topology owner filters must be repository-relative and traversal-free"
                .to_owned(),
        });
    }
    Ok(block)
}

fn invalid_path_filter(value: &str) -> bool {
    let path = value.trim_end_matches('/');
    path.is_empty()
        || value.starts_with('/')
        || value.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}
