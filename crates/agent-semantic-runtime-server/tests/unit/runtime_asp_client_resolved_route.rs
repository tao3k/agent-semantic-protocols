use super::{selected_playbook_provider_targets, selected_provider_targets};

#[test]
fn pipe_order_is_preserved_and_duplicates_are_removed() {
    let installed = vec![
        ("python".to_owned(), "asp-python".to_owned()),
        ("rust".to_owned(), "asp-rust".to_owned()),
    ];
    let targets = selected_provider_targets(Some("rust|python|rust"), &installed)
        .unwrap_or_else(|_| panic!("selected targets"));

    assert_eq!(
        targets
            .iter()
            .map(|target| target.language_id.as_str())
            .collect::<Vec<_>>(),
        vec!["rust", "python"]
    );
}

#[test]
fn unrelated_uninstalled_provider_is_not_part_of_selected_closure() {
    let installed = vec![("rust".to_owned(), "asp-rust".to_owned())];
    let targets = selected_provider_targets(Some("rust"), &installed)
        .unwrap_or_else(|_| panic!("selected target"));

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].provider_id.as_deref(), Some("asp-rust"));
}

#[test]
fn playbook_language_set_is_admitted_before_generation_work() {
    let providers = vec![("org".to_owned(), "embedded-org".to_owned())];
    assert!(selected_playbook_provider_targets("org", &providers).is_ok());
    let error = selected_playbook_provider_targets("rust", &providers)
        .expect_err("an uninstalled producer cannot enter generation work");
    let super::AspClientOperationError::Message(message) = error else {
        panic!("producer rejection must remain a local typed message")
    };
    assert!(message.contains("producer is not installed"), "{message}");
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
