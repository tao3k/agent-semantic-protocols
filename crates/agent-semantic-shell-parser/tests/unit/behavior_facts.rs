// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::CommandStage;
use super::ShellAccessKind;
use super::ShellBehaviorEvidence;
use super::ShellBehaviorFact;
use super::command_stage_behavior_facts;

#[test]
fn projects_input_output_and_read_write_syntax() {
    let stage = CommandStage::new(vec![
        "tool".into(),
        "<".into(),
        "input.rs".into(),
        ">out.json".into(),
        "3<>state.db".into(),
    ]);
    assert_eq!(
        command_stage_behavior_facts(&stage),
        vec![
            ShellBehaviorFact {
                access: ShellAccessKind::Read,
                evidence: ShellBehaviorEvidence::InputRedirection,
                subject: Some("input.rs".into()),
            },
            ShellBehaviorFact {
                access: ShellAccessKind::Write,
                evidence: ShellBehaviorEvidence::OutputRedirection,
                subject: Some("out.json".into()),
            },
            ShellBehaviorFact {
                access: ShellAccessKind::Read,
                evidence: ShellBehaviorEvidence::ReadWriteRedirection,
                subject: Some("state.db".into()),
            },
            ShellBehaviorFact {
                access: ShellAccessKind::Write,
                evidence: ShellBehaviorEvidence::ReadWriteRedirection,
                subject: Some("state.db".into()),
            },
        ]
    );
}

#[test]
fn heredoc_and_here_string_are_not_filesystem_reads() {
    for words in [
        vec!["tool".into(), "<<".into(), "EOF".into()],
        vec!["tool".into(), "<<<payload".into()],
    ] {
        assert!(command_stage_behavior_facts(&CommandStage::new(words)).is_empty());
    }
}

#[test]
fn parsed_bash_redirections_survive_into_behavior_facts() {
    let stages = crate::parse_bash_command_candidates("opaque < input.rs > output.rs <> state.db")
        .expect("parse Bash redirections");
    let facts = stages
        .iter()
        .flat_map(command_stage_behavior_facts)
        .collect::<Vec<_>>();
    assert!(facts.iter().any(|fact| {
        fact.access == ShellAccessKind::Read && fact.subject.as_deref() == Some("input.rs")
    }));
    assert!(facts.iter().any(|fact| {
        fact.access == ShellAccessKind::Write && fact.subject.as_deref() == Some("output.rs")
    }));
    assert!(facts.iter().any(|fact| {
        fact.access == ShellAccessKind::Read && fact.subject.as_deref() == Some("state.db")
    }));
    assert!(facts.iter().any(|fact| {
        fact.access == ShellAccessKind::Write && fact.subject.as_deref() == Some("state.db")
    }));
}
