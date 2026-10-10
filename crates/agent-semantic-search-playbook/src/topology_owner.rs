// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Scheme lowering for generation-bound Topology owner membership.

use agent_semantic_scheme_syntax::SchemeDatum;

use crate::{ProgressiveSearchPlaybookError, TopologyOwnerMembershipBlock};

pub(crate) fn parse(
    values: &[SchemeDatum],
) -> Result<TopologyOwnerMembershipBlock, ProgressiveSearchPlaybookError> {
    let [SchemeDatum::List(owners)] = values else {
        return Err(invalid(
            "search-playbook-topology-invalid",
            "topology requires exactly one (owners ...) predicate",
        ));
    };
    if symbol(owners.first()) != Some("owners") {
        return Err(invalid(
            "search-playbook-topology-invalid",
            "topology requires an owners predicate",
        ));
    }
    let mut block = TopologyOwnerMembershipBlock {
        kind: String::new(),
        exact_path: None,
        path_prefix: None,
        extension: None,
        path_glob: None,
    };
    for predicate in &owners[1..] {
        lower_predicate(predicate, &mut block)?;
    }
    validate_block(block)
}

fn lower_predicate(
    predicate: &SchemeDatum,
    block: &mut TopologyOwnerMembershipBlock,
) -> Result<(), ProgressiveSearchPlaybookError> {
    let SchemeDatum::List(predicate) = predicate else {
        return Err(invalid(
            "search-playbook-topology-invalid",
            "topology owner predicates must be lists",
        ));
    };
    let Some(name) = symbol(predicate.first()) else {
        return Err(invalid(
            "search-playbook-topology-invalid",
            "topology owner predicate must begin with a field name",
        ));
    };
    match name {
        "kind" => lower_kind(predicate, block),
        "path" | "path-prefix" | "extension" | "path-glob" => lower_filter(name, predicate, block),
        other => Err(ProgressiveSearchPlaybookError::UnsupportedOption(
            other.to_owned(),
        )),
    }
}

fn lower_kind(
    predicate: &[SchemeDatum],
    block: &mut TopologyOwnerMembershipBlock,
) -> Result<(), ProgressiveSearchPlaybookError> {
    let [_, SchemeDatum::Symbol(kind)] = predicate else {
        return Err(invalid(
            "search-playbook-topology-kind-invalid",
            "topology owner kind requires the symbol file",
        ));
    };
    if kind != "file" || !block.kind.is_empty() {
        return Err(invalid(
            "search-playbook-topology-kind-invalid",
            "topology owner kind must occur once and equal file",
        ));
    }
    block.kind.clone_from(kind);
    Ok(())
}

fn lower_filter(
    name: &str,
    predicate: &[SchemeDatum],
    block: &mut TopologyOwnerMembershipBlock,
) -> Result<(), ProgressiveSearchPlaybookError> {
    let [_, SchemeDatum::String(value)] = predicate else {
        return Err(invalid(
            "search-playbook-topology-filter-invalid",
            format!("topology owner {name} requires one string"),
        ));
    };
    let target = match name {
        "path" => &mut block.exact_path,
        "path-prefix" => &mut block.path_prefix,
        "extension" => &mut block.extension,
        "path-glob" => &mut block.path_glob,
        _ => unreachable!(),
    };
    if target.replace(value.clone()).is_some() {
        return Err(invalid(
            "search-playbook-topology-filter-invalid",
            format!("topology owner {name} may occur only once"),
        ));
    }
    Ok(())
}

fn validate_block(
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
        return Err(invalid(
            "search-playbook-topology-filter-invalid",
            "topology owner filters must be repository-relative and traversal-free",
        ));
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

fn symbol(value: Option<&SchemeDatum>) -> Option<&str> {
    match value {
        Some(SchemeDatum::Symbol(value)) => Some(value),
        _ => None,
    }
}

fn invalid(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> ProgressiveSearchPlaybookError {
    ProgressiveSearchPlaybookError::InvalidParameter {
        reason_kind,
        message: message.into(),
    }
}
