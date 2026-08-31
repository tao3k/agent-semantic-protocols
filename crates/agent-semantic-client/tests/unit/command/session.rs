use super::{
    CodexThreadBinding, RegisterChildArgs, parse_register_child_args, resolve_codex_thread_binding,
};

#[test]
fn registration_parser_requires_explicit_parent_and_agent() {
    let args = parse_register_child_args(&[
        "--parent-thread-id".to_owned(),
        "parent-1".to_owned(),
        "--agent-name".to_owned(),
        "asp_explorer".to_owned(),
    ])
    .expect("explicit child registration command must parse");
    assert_eq!(
        args,
        RegisterChildArgs {
            parent_thread_id: "parent-1".to_owned(),
            agent_name: "asp_explorer".to_owned(),
        }
    );
    assert!(parse_register_child_args(&[]).is_err());
}

#[test]
fn child_identity_separates_root_session_parent_thread_and_child_thread() {
    assert_eq!(
        resolve_codex_thread_binding(
            "parent-1",
            Some("child-1".to_owned()),
            Some("root-1".to_owned())
        ),
        Ok(CodexThreadBinding {
            root_session_id: "root-1".to_owned(),
            parent_thread_id: "parent-1".to_owned(),
            child_thread_id: "child-1".to_owned(),
        })
    );
    assert!(
        resolve_codex_thread_binding(
            "parent-1",
            Some("parent-1".to_owned()),
            Some("root-1".to_owned())
        )
        .is_err()
    );
    assert!(
        resolve_codex_thread_binding(
            "parent-1",
            Some("parent-1".to_owned()),
            Some("parent-1".to_owned())
        )
        .is_err()
    );
    assert!(resolve_codex_thread_binding("parent-1", None, None).is_err());
}
