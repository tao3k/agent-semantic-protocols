#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandInvocationShapeV1 {
    Command,
    WrappedCommand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandWrapperMatchV1 {
    Matched,
    Unmatched,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandFlagPresenceV1 {
    Present,
    Absent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandWrapperSpecV1 {
    pub executable: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCommandInvocationV1 {
    pub shape: CommandInvocationShapeV1,
    pub wrapper_match: CommandWrapperMatchV1,
    pub wrapper_chain: Vec<String>,
    pub argv: Vec<String>,
    pub executable: Option<String>,
    pub flags: Vec<String>,
    pub flag_presence: CommandFlagPresenceV1,
    pub operands: Vec<String>,
}

pub fn normalize_bash_command_invocations(
    command: &str,
    wrappers: &[CommandWrapperSpecV1],
) -> Result<Vec<SemanticCommandInvocationV1>, String> {
    crate::parse_bash_command_candidates(command).map(|stages| {
        stages
            .into_iter()
            .filter_map(|stage| normalize_stage(stage, wrappers))
            .collect()
    })
}

fn is_environment_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn normalize_stage(
    stage: crate::CommandStageV1,
    wrappers: &[CommandWrapperSpecV1],
) -> Option<SemanticCommandInvocationV1> {
    let crate::CommandStageV1 { words } = stage;
    if words.is_empty() {
        return None;
    }

    let mut cursor = 0;
    let mut wrapper_chain = Vec::new();
    let mut flags = Vec::new();

    while cursor < words.len() && is_registered_wrapper(&words[cursor], wrappers) {
        wrapper_chain.push(words[cursor].clone());
        cursor += 1;
        while cursor < words.len() && is_flag(&words[cursor]) {
            flags.push(words[cursor].clone());
            cursor += 1;
        }
        while cursor < words.len() && is_environment_assignment(&words[cursor]) {
            cursor += 1;
        }
    }

    while cursor < words.len() && is_environment_assignment(&words[cursor]) {
        cursor += 1;
    }

    let argv = words[cursor..].to_vec();
    let executable = words.get(cursor).cloned();
    if executable.is_some() {
        cursor += 1;
    }

    let mut operands = Vec::new();
    for word in &words[cursor..] {
        if is_flag(word) {
            flags.push(word.clone());
        } else {
            operands.push(word.clone());
        }
    }

    let wrapper_match = if wrapper_chain.is_empty() {
        CommandWrapperMatchV1::Unmatched
    } else {
        CommandWrapperMatchV1::Matched
    };
    let shape = if wrapper_chain.is_empty() {
        CommandInvocationShapeV1::Command
    } else {
        CommandInvocationShapeV1::WrappedCommand
    };
    let flag_presence = if flags.is_empty() {
        CommandFlagPresenceV1::Absent
    } else {
        CommandFlagPresenceV1::Present
    };

    Some(SemanticCommandInvocationV1 {
        shape,
        wrapper_match,
        wrapper_chain,
        argv,
        executable,
        flags,
        flag_presence,
        operands,
    })
}

pub fn semantic_invocations_match_prefix(
    invocations: &[SemanticCommandInvocationV1],
    prefix: &[String],
) -> crate::PrefixMatch {
    if prefix.is_empty() {
        return crate::PrefixMatch::Matched;
    }
    let mut inspected_candidates = 0usize;
    for invocation in invocations {
        if invocation.argv.len() > crate::MAX_STAGE_TOKENS {
            return crate::PrefixMatch::BudgetExceeded;
        }
        if invocation.argv.len() < prefix.len() {
            continue;
        }
        if inspected_candidates == crate::MAX_COMMAND_CANDIDATES {
            return crate::PrefixMatch::BudgetExceeded;
        }
        inspected_candidates += 1;
        if crate::candidate_matches_prefix(&invocation.argv, prefix) {
            return crate::PrefixMatch::Matched;
        }
    }
    crate::PrefixMatch::NotMatched
}

fn is_registered_wrapper(token: &str, wrappers: &[CommandWrapperSpecV1]) -> bool {
    let basename = crate::command_token_basename(token);
    wrappers
        .iter()
        .any(|wrapper| crate::command_token_basename(&wrapper.executable) == basename)
}

fn is_flag(token: &str) -> bool {
    token != "-" && token.starts_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let invocations =
            normalize_bash_command_invocations("rtk -q read --number *.rs", &wrappers)
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
}
