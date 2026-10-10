// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Provider target admission for resolved Search and Query routes.

use super::AspClientOperationError;

pub(super) fn selected_playbook_provider_targets(
    language: Option<&str>,
    documents: Option<&str>,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
) -> Result<
    Vec<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget>,
    AspClientOperationError,
> {
    let provider_by_language = providers
        .iter()
        .map(|provider| (provider.language_id.as_str(), provider))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut requested = language
        .into_iter()
        .flat_map(|expression| expression.split('|'))
        .map(|producer| {
            (
                producer.trim(),
                agent_semantic_search::WorkspaceSearchProducerAxis::Language,
            )
        })
        .chain(
            documents
                .into_iter()
                .flat_map(|expression| expression.split('|'))
                .map(|producer| {
                    (
                        producer.trim(),
                        agent_semantic_search::WorkspaceSearchProducerAxis::Document,
                    )
                }),
        )
        .filter(|(producer, _)| !producer.is_empty())
        .collect::<Vec<_>>();
    requested.sort_unstable();
    requested.dedup();
    let mut admitted = std::collections::BTreeSet::new();
    let mut targets = Vec::new();
    for (producer, producer_axis) in requested {
        let provider = provider_by_language.get(producer).copied().ok_or_else(|| {
            AspClientOperationError::Message(format!(
                "Search Playbook producer is not installed: producer={producer}"
            ))
        })?;
        if !provider.producer_axes.contains(&producer_axis) {
            return Err(AspClientOperationError::Message(format!(
                "Playbook producer is declared on the wrong axis: producer={producer} requestedAxis={producer_axis:?} admittedAxes={:?}",
                provider.producer_axes
            )));
        }
        if admitted.insert((producer.to_owned(), provider.provider_id.clone())) {
            targets.push(
                agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                    language_id: producer.to_owned(),
                    provider_id: match producer_axis {
                        agent_semantic_search::WorkspaceSearchProducerAxis::Language => {
                            Some(provider.provider_id.clone())
                        }
                        agent_semantic_search::WorkspaceSearchProducerAxis::Document => None,
                    },
                },
            );
        }
    }
    Ok(targets)
}

pub(super) fn selected_syntax_plan_provider_targets(
    producer: &str,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
) -> Result<
    Vec<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget>,
    AspClientOperationError,
> {
    let provider = providers
        .iter()
        .find(|provider| provider.language_id == producer)
        .ok_or_else(|| {
            AspClientOperationError::Message(format!(
                "syntax plan producer is not admitted: producer={producer}",
            ))
        })?;
    match provider.producer_axes.as_slice() {
        [agent_semantic_search::WorkspaceSearchProducerAxis::Language] => {
            selected_playbook_provider_targets(Some(producer), None, providers)
        }
        [agent_semantic_search::WorkspaceSearchProducerAxis::Document] => {
            selected_playbook_provider_targets(None, Some(producer), providers)
        }
        _ => Err(AspClientOperationError::Message(format!(
            "syntax plan producer axis is ambiguous: producer={producer}",
        ))),
    }
}
