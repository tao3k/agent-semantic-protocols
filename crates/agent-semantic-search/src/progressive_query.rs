// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure typed exact-selector Query Playbook admission for Marlin-native ASP.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Presentation selected for a Query result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueryOutputFormat {
    Human,
    Json,
}

/// Pure typed input projected from Marlin's Gerbil Scheme module system.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressiveQueryInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub documents: Vec<String>,
    pub selectors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_format: Option<QueryOutputFormat>,
}

/// Validated selector-addressed Query request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProgressiveQueryRequest {
    Selector {
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        documents: Option<String>,
        selectors: Vec<String>,
        projection: String,
        output_format: QueryOutputFormat,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
    },
}

/// Admit a pure typed selector query without pseudo S-expression text parsing.
pub fn admit_progressive_query(
    input: ProgressiveQueryInput,
) -> Result<ProgressiveQueryRequest, String> {
    if let Some(workspace) = &input.workspace
        && !valid_registered_name(workspace)
    {
        return Err("workspace requires one registered WorkspaceId, not a path".to_owned());
    }
    let language = normalize_producers(&input.languages)?;
    let documents = normalize_producers(&input.documents)?;
    if language.is_none() && documents.is_none() {
        return Err("at least one producer axis is required".to_owned());
    }
    if input.selectors.is_empty() {
        return Err("selectors requires at least one canonical selector".to_owned());
    }
    let allowed = input
        .languages
        .iter()
        .chain(input.documents.iter())
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for selector in &input.selectors {
        let Some((producer, rest)) = selector.split_once("://") else {
            return Err(format!("invalid canonical selector: {selector}"));
        };
        if !valid_registered_name(producer) || rest.is_empty() {
            return Err(format!("invalid canonical selector: {selector}"));
        }
        if !allowed.contains(producer) {
            return Err(format!(
                "selector producer `{producer}` is outside the declared producer set"
            ));
        }
        if !seen.insert(selector.as_str()) {
            return Err("selectors must be unique".to_owned());
        }
    }
    let projection = input.projection.unwrap_or_else(|| "source".to_owned());
    if !matches!(projection.as_str(), "source" | "callable-skeleton") {
        return Err(format!(
            "unsupported query projection `{projection}`; expected source or callable-skeleton"
        ));
    }
    Ok(ProgressiveQueryRequest::Selector {
        language,
        documents,
        selectors: input.selectors,
        projection,
        output_format: input.output_format.unwrap_or(QueryOutputFormat::Human),
        workspace: input.workspace,
    })
}

fn normalize_producers(values: &[String]) -> Result<Option<String>, String> {
    if values.is_empty() {
        return Ok(None);
    }
    if values.iter().any(|value| !valid_registered_name(value)) {
        return Err("producer values must be registered-name symbols".to_owned());
    }
    Ok(Some(values.join("|")))
}

fn valid_registered_name(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
}

#[cfg(test)]
#[path = "../tests/unit/progressive_query.rs"]
mod tests;
