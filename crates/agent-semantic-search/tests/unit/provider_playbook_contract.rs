use super::{
    ProviderSearchPlaybookContract, ProviderSearchPlaybookProjection, SearchPlaybookContractResult,
    resolve_provider_search_playbook_contract,
};

fn provider(contract: Option<ProviderSearchPlaybookContract>) -> crate::WorkspaceSearchProvider {
    crate::WorkspaceSearchProvider {
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        source_extensions: vec!["rs".to_owned()],
        search_supported: true,
        search_playbook_contract: contract,
    }
}

#[test]
fn missing_contract_fails_without_a_fallback_projection() {
    let providers = [provider(None)];
    let receipt = resolve_provider_search_playbook_contract(&providers, Some("rust"), None);
    assert_eq!(
        receipt.result,
        SearchPlaybookContractResult::ProviderContractFailure
    );
    assert!(receipt.projection.is_none());
}

#[test]
fn admitted_contract_projects_exact_provider_text() {
    let projection = ProviderSearchPlaybookProjection {
        example: "asp search playbook --languages rust ...".to_owned(),
        grammar: "asp search playbook --languages rust ...".to_owned(),
    };
    let providers = [provider(Some(ProviderSearchPlaybookContract {
        contract_id: "asp-rust.search-playbook".to_owned(),
        contract_version: "1".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        syntax_contract_id: "tree-sitter-rust".to_owned(),
        syntax_contract_digest: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_owned(),
        projection: projection.clone(),
    }))];
    let receipt = resolve_provider_search_playbook_contract(&providers, Some("rust"), None);
    assert_eq!(receipt.result, SearchPlaybookContractResult::Ready);
    assert_eq!(receipt.projection, Some(projection));
    let encoded = serde_json::to_value(receipt).expect("contract receipt");
    assert!(encoded.get("syntaxContractDigest").is_none());
    assert!(encoded.get("providerId").is_none());
}

#[test]
fn pipe_composition_preserves_requested_reasoning_order() {
    let rust = provider(Some(ProviderSearchPlaybookContract {
        contract_id: "asp-rust.search-playbook".to_owned(),
        contract_version: "1".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        syntax_contract_id: "tree-sitter-rust".to_owned(),
        syntax_contract_digest: "0".repeat(64),
        projection: ProviderSearchPlaybookProjection {
            example: "rust-example".to_owned(),
            grammar: "rust-grammar".to_owned(),
        },
    }));
    let mut python = rust.clone();
    python.language_id = "python".to_owned();
    python.provider_id = "asp-python".to_owned();
    let contract = python
        .search_playbook_contract
        .as_mut()
        .expect("python contract");
    contract.language_id = "python".to_owned();
    contract.provider_id = "asp-python".to_owned();
    contract.projection.example = "python-example".to_owned();
    contract.projection.grammar = "python-grammar".to_owned();

    let receipt =
        resolve_provider_search_playbook_contract([&rust, &python], Some("python|rust"), None);
    assert_eq!(receipt.requested_producers, ["python", "rust"]);
    assert_eq!(
        receipt.projection.expect("composed projection"),
        ProviderSearchPlaybookProjection {
            example: "python-example\nrust-example".to_owned(),
            grammar: "python-grammar\nrust-grammar".to_owned(),
        }
    );
}
