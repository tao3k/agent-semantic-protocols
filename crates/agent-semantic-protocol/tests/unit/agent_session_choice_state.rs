use super::{HostLifecycleSurface, resolve_agent_session_choice_state};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransitionFixture {
    schema_id: String,
    schema_version: String,
    cases: Vec<TransitionCase>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransitionCase {
    namespace_state: String,
    host_surface: String,
    expected_state: String,
}

#[test]
fn missing_namespace_is_terminal_when_host_lifecycle_surface_is_unavailable() {
    let state = resolve_agent_session_choice_state(
        "registration-required",
        None,
        HostLifecycleSurface::Unavailable,
    );
    assert_eq!(state.state, "host-surface-unavailable");
    assert_eq!(
        state.reason_kind,
        Some("host-lifecycle-surface-unavailable")
    );
}

#[test]
fn missing_namespace_remains_creatable_when_host_lifecycle_surface_is_available() {
    let state = resolve_agent_session_choice_state(
        "registration-required",
        None,
        HostLifecycleSurface::Available,
    );
    assert_eq!(state.state, "registration-required");
    assert_eq!(state.reason_kind, None);
}

#[test]
fn existing_namespace_does_not_depend_on_host_surface_discovery() {
    for namespace_state in ["registered", "resumable", "achieved", "blocked"] {
        let state = resolve_agent_session_choice_state(
            namespace_state,
            Some("preserved-reason"),
            HostLifecycleSurface::Unavailable,
        );
        assert_eq!(state.state, namespace_state);
        assert_eq!(state.reason_kind, Some("preserved-reason"));
    }
}

#[test]
fn versioned_transition_fixture_matches_the_executable_state_machine() {
    let fixture: TransitionFixture = serde_json::from_str(include_str!(
        "../fixtures/agent_session_choice_state.v1.json"
    ))
    .expect("parse transition fixture");
    assert_eq!(
        fixture.schema_id,
        "agent.semantic-protocols.agent-session-choice-transition.v1"
    );
    assert_eq!(fixture.schema_version, "1");
    for case in fixture.cases {
        let host_surface = match case.host_surface.as_str() {
            "available" => HostLifecycleSurface::Available,
            "unavailable" => HostLifecycleSurface::Unavailable,
            other => panic!("unknown host surface `{other}`"),
        };
        let actual =
            resolve_agent_session_choice_state(case.namespace_state.as_str(), None, host_surface);
        assert_eq!(
            actual.state, case.expected_state,
            "{}",
            case.namespace_state
        );
    }
}
