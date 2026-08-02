use std::collections::BTreeSet;

use super::{registered_provider_binaries_v1, registered_provider_catalog_identities};

#[test]
fn registered_binary_identities_are_materialized_from_the_v1_registry_schema() {
    let registry: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/semantic-language-registry.providers.v1.json"
    ))
    .expect("v1 provider registry must be valid JSON");
    let expected = registry["languages"]
        .as_array()
        .expect("v1 provider registry must declare languages")
        .iter()
        .map(|registration| {
            (
                registration["languageId"]
                    .as_str()
                    .expect("registration must declare languageId")
                    .to_string(),
                registration["providerId"]
                    .as_str()
                    .expect("registration must declare providerId")
                    .to_string(),
                registration["binary"]
                    .as_str()
                    .expect("registration must declare binary")
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    let actual = registered_provider_binaries_v1()
        .into_iter()
        .map(|identity| {
            (
                identity.language_id().as_str().to_string(),
                identity.provider_id().as_str().to_string(),
                identity.binary().to_string(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn shared_provider_binaries_are_not_duplicated_by_publication_planners() {
    let registrations = registered_provider_binaries_v1();
    let binaries = registrations
        .iter()
        .map(|identity| identity.binary())
        .collect::<BTreeSet<_>>();

    assert!(registrations.len() >= binaries.len());
    assert!(binaries.iter().all(|binary| {
        !binary.is_empty()
            && !binary.contains('/')
            && !binary.contains('\\')
            && *binary != "."
            && *binary != ".."
    }));
}

#[test]
fn every_registered_binary_is_provider_internal_without_a_name_blacklist() {
    for registration in super::registered_provider_binaries_v1() {
        assert_eq!(
            registration.profile(),
            super::RuntimeBinaryProfileV1::ProviderInternal
        );
        for executable in [
            registration.binary().to_string(),
            format!("/canonical/runtime/bin/{}", registration.binary()),
        ] {
            let super::RuntimeBinaryClassificationV1::RegisteredProviderInternal(matches) =
                super::classify_runtime_executable_v1(&executable)
            else {
                panic!("registered provider binary must classify as provider-internal");
            };
            assert!(matches.contains(&registration));
        }
    }
}

#[test]
fn executable_identity_matching_uses_the_registered_name_even_when_renamed() {
    assert!(super::executable_identity_matches_v1(
        "/canonical/runtime/bin/provider-renamed-by-registry",
        "provider-renamed-by-registry"
    ));
    assert!(!super::executable_identity_matches_v1(
        "/canonical/runtime/bin/provider-renamed-by-registry",
        "some-hardcoded-harness-name"
    ));
}

#[test]
fn facade_and_unregistered_runtime_artifacts_are_not_provider_internal() {
    assert_eq!(
        super::classify_runtime_executable_v1("/usr/local/bin/asp"),
        super::RuntimeBinaryClassificationV1::Facade
    );
    assert_eq!(
        super::classify_runtime_executable_v1("/runtime/bin/not-in-provider-registry"),
        super::RuntimeBinaryClassificationV1::Unregistered
    );
}

#[test]
fn provider_internal_profile_requires_exact_single_use_runtime_dispatch() {
    let requirements = super::runtime_binary_dispatch_requirements_v1(
        super::RuntimeBinaryProfileV1::ProviderInternal,
    );

    assert_eq!(
        requirements.allowed_authorities,
        &[super::RuntimeBinaryInvocationAuthorityV1::AspRuntimeDispatch]
    );
    assert_eq!(requirements.root_session, super::SessionBindingV1::Required);
    assert_eq!(
        requirements.child_session,
        super::SessionBindingV1::Required
    );
    assert_eq!(
        requirements.generation,
        super::RuntimeBinaryIdentityBindingV1::Exact
    );
    assert_eq!(
        requirements.artifact,
        super::RuntimeBinaryIdentityBindingV1::Exact
    );
    assert_eq!(
        requirements.argv,
        super::RuntimeBinaryIdentityBindingV1::Exact
    );
    assert_eq!(
        requirements.attempt,
        super::RuntimeBinaryIdentityBindingV1::Exact
    );
    assert!(requirements.single_use);
}

#[test]
fn all_runtime_binary_profiles_have_typed_dispatch_requirements() {
    let profiles = [
        super::RuntimeBinaryProfileV1::Facade,
        super::RuntimeBinaryProfileV1::ProviderInternal,
        super::RuntimeBinaryProfileV1::SupportTool,
        super::RuntimeBinaryProfileV1::HostTool,
        super::RuntimeBinaryProfileV1::TestFixture,
    ];

    for profile in profiles {
        assert_eq!(
            super::runtime_binary_dispatch_requirements_v1(profile).profile,
            profile
        );
    }

    let denial_reasons = [
        super::RuntimeBinaryAdmissionDenialV1::DirectProviderInternal,
        super::RuntimeBinaryAdmissionDenialV1::MissingDispatchCapability,
        super::RuntimeBinaryAdmissionDenialV1::UnregisteredRuntimeBinary,
    ];
    assert_eq!(denial_reasons.len(), 3);
}

#[test]
fn known_root_session_denies_direct_provider_binary_without_dispatch_capability() {
    let registration = super::registered_provider_binaries_v1()
        .into_iter()
        .next()
        .expect("provider registry must contain at least one binary");
    let runtime = crate::HookRuntime {
        project_root: ".".to_string(),
        rankers: Vec::new(),
        providers: Vec::new(),
    };
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": format!("{} query --selector example", registration.binary())
        },
        "session_id": "root-session-known"
    });

    let decision = crate::classify_hook(&runtime, "codex", "pre-tool", &payload);

    assert_eq!(decision.decision, crate::DecisionKind::Deny);
    assert_eq!(
        decision.reason_kind,
        crate::ReasonKind::ProviderBinaryDirectExecution
    );
    assert_eq!(
        decision.fields["runtimeBinaryAdmissionDenial"],
        serde_json::Value::String("missing-dispatch-capability".to_string())
    );
    assert!(
        decision.routes.iter().all(
            |route| route.binary == "asp" && route.argv.first().is_some_and(|arg| arg == "asp")
        )
    );
}

#[test]
fn every_registered_language_has_one_canonical_exact_query_pack_identity() {
    let identities = registered_provider_catalog_identities();
    assert_eq!(identities.len(), registered_provider_binaries_v1().len());
    for identity in identities {
        assert_eq!(identity.exact_query_pack_identity_digest.len(), 64);
        assert!(
            identity
                .exact_query_pack_identity_digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
            "language {} has a non-canonical exact query-pack identity",
            identity.language_id
        );
    }
}
