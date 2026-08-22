use super::{
    CommandFlagPresenceV1, CommandInvocationShapeV1, CommandWrapperMatchV1, CommandWrapperSpecV1,
    normalize_bash_command_invocations, semantic_invocations_match_prefix,
};

#[test]
fn preserves_flags_and_unexpanded_operands_for_direct_commands() {
    let invocations = normalize_bash_command_invocations("reader --number *.rs", &[])
        .expect("bash command should parse");

    assert_eq!(invocations.len(), 1);
    let invocation = &invocations[0];
    assert_eq!(invocation.shape, CommandInvocationShapeV1::Command);
    assert_eq!(invocation.wrapper_match, CommandWrapperMatchV1::Unmatched);
    assert_eq!(invocation.executable.as_deref(), Some("reader"));
    assert_eq!(invocation.flag_presence, CommandFlagPresenceV1::Present);
    assert_eq!(invocation.flags, ["--number"]);
    assert_eq!(invocation.operands, ["*.rs"]);
}

#[test]
fn wrapper_registry_exposes_inner_execute_without_command_vocabulary() {
    let wrappers = [CommandWrapperSpecV1 {
        executable: "rtk".to_string(),
    }];
    let invocations = normalize_bash_command_invocations("rtk read *.rs", &wrappers)
        .expect("wrapped bash command should parse");

    let invocation = invocations
        .iter()
        .find(|invocation| invocation.shape == CommandInvocationShapeV1::WrappedCommand)
        .expect("wrapper registry should produce a wrapped invocation candidate");
    assert_eq!(invocation.shape, CommandInvocationShapeV1::WrappedCommand);
    assert_eq!(invocation.wrapper_match, CommandWrapperMatchV1::Matched);
    assert_eq!(invocation.wrapper_chain, ["rtk"]);
    assert_eq!(invocation.executable.as_deref(), Some("read"));
    assert_eq!(invocation.flag_presence, CommandFlagPresenceV1::Absent);
    assert_eq!(invocation.operands, ["*.rs"]);
}

#[test]
fn wrapper_and_inner_flags_do_not_hide_source_operands() {
    let wrappers = [CommandWrapperSpecV1 {
        executable: "rtk".to_string(),
    }];
    let invocations = normalize_bash_command_invocations("rtk -q read --number *.rs", &wrappers)
        .expect("wrapped bash command with flags should parse");

    let invocation = &invocations[0];
    assert_eq!(invocation.flags, ["-q", "--number"]);
    assert_eq!(invocation.flag_presence, CommandFlagPresenceV1::Present);
    assert_eq!(invocation.operands, ["*.rs"]);
}

#[test]
fn normalized_prefix_matching_preserves_rename_identity_across_real_wrappers() {
    let wrappers = [CommandWrapperSpecV1 {
        executable: "rtk".to_string(),
    }];
    let scenarios = [
        "git mv crates/old.rs crates/new.rs",
        "/usr/bin/git mv crates/old.rs crates/new.rs",
        "rtk git mv crates/old.rs crates/new.rs",
        "LANG=C git mv crates/old.rs crates/new.rs",
    ];
    let rename_prefix = ["git".to_string(), "mv".to_string()];
    let read_prefix = ["cat".to_string()];

    for command in scenarios {
        let invocations = normalize_bash_command_invocations(command, &wrappers)
            .expect("real git mv scenario should parse");
        assert_eq!(
            semantic_invocations_match_prefix(&invocations, &rename_prefix),
            crate::PrefixMatch::Matched,
            "rename prefix should survive normalization for {command}"
        );
        assert_eq!(
            semantic_invocations_match_prefix(&invocations, &read_prefix),
            crate::PrefixMatch::NotMatched,
            "rename must not acquire read identity for {command}"
        );
    }
}
