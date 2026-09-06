//! Provider-owned Search Playbook contract admission and projection.

use serde::{Deserialize, Serialize};

pub const SEARCH_PLAYBOOK_CONTRACT_PROJECTION_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-playbook-contract-projection";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSearchPlaybookProjection {
    pub example: String,
    pub grammar: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSearchPlaybookContract {
    pub contract_id: String,
    pub contract_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub syntax_contract_id: String,
    pub syntax_contract_digest: String,
    pub projection: ProviderSearchPlaybookProjection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchPlaybookContractResult {
    Ready,
    ProviderContractFailure,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookContractProjectionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub result: SearchPlaybookContractResult,
    pub requested_producers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection: Option<ProviderSearchPlaybookProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub fn resolve_provider_search_playbook_contract<'a>(
    providers: impl IntoIterator<Item = &'a crate::WorkspaceSearchProvider>,
    languages: Option<&str>,
    documents: Option<&str>,
) -> SearchPlaybookContractProjectionReceipt {
    let requested_producers = languages
        .into_iter()
        .chain(documents)
        .flat_map(|expression| expression.split('|'))
        .fold(Vec::<String>::new(), |mut producers, producer| {
            if !producers.iter().any(|known| known == producer) {
                producers.push(producer.to_owned());
            }
            producers
        });
    let providers = providers.into_iter().collect::<Vec<_>>();

    let failure = |reason: &str| SearchPlaybookContractProjectionReceipt {
        schema_id: SEARCH_PLAYBOOK_CONTRACT_PROJECTION_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        result: SearchPlaybookContractResult::ProviderContractFailure,
        requested_producers: requested_producers.clone(),
        projection: None,
        reason: Some(reason.to_owned()),
    };

    if requested_producers.is_empty() {
        return failure("producer-selector-required");
    }

    let mut examples = Vec::with_capacity(requested_producers.len());
    let mut grammars = Vec::with_capacity(requested_producers.len());
    for producer in &requested_producers {
        let Some(provider) = providers
            .iter()
            .find(|provider| provider.language_id == *producer)
        else {
            return failure("provider-not-registered");
        };
        let Some(contract) = provider.search_playbook_contract.as_ref() else {
            return failure("provider-search-playbook-contract-unavailable");
        };
        if contract.language_id != provider.language_id
            || contract.provider_id != provider.provider_id
        {
            return failure("provider-search-playbook-contract-identity-mismatch");
        }
        examples.push(contract.projection.example.as_str());
        grammars.push(contract.projection.grammar.as_str());
    }

    SearchPlaybookContractProjectionReceipt {
        schema_id: SEARCH_PLAYBOOK_CONTRACT_PROJECTION_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        result: SearchPlaybookContractResult::Ready,
        requested_producers,
        projection: Some(ProviderSearchPlaybookProjection {
            example: examples.join("\n"),
            grammar: grammars.join("\n"),
        }),
        reason: None,
    }
}

#[cfg(test)]
#[path = "../tests/unit/provider_playbook_contract.rs"]
mod tests;
