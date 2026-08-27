//! Parser-owned semantic behavior facts derived from shell syntax.

use crate::CommandStage;

/// A filesystem access capability proven by shell syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellAccessKind {
    /// The stage can read bytes from a filesystem subject.
    Read,
    /// The stage can write bytes to a filesystem subject.
    Write,
}

/// The shell grammar construct that proves a behavior fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellBehaviorEvidence {
    /// An input redirection such as `< path`.
    InputRedirection,
    /// An output redirection such as `> path`, `>> path`, or `>| path`.
    OutputRedirection,
    /// A read/write redirection such as `<> path`.
    ReadWriteRedirection,
}

/// A semantic capability projected from one command stage by shell grammar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellBehaviorFact {
    /// The filesystem capability proven by the syntax.
    pub access: ShellAccessKind,
    /// The syntax evidence that established the capability.
    pub evidence: ShellBehaviorEvidence,
    /// The redirection subject when the parser exposed one.
    pub subject: Option<String>,
}

/// Projects filesystem behavior from redirection syntax in a parsed command stage.
///
/// Here-documents and here-strings are intentionally excluded: their bodies are shell-owned
/// literal input, not evidence that a filesystem path is read.
pub fn command_stage_behavior_facts(stage: &CommandStage) -> Vec<ShellBehaviorFact> {
    let words = stage.words();
    let mut facts = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let Some((operator, inline_subject)) = redirection_token(word) else {
            continue;
        };
        let subject = inline_subject.map(str::to_owned).or_else(|| {
            words
                .get(index + 1)
                .filter(|next| !is_operator(next))
                .cloned()
        });
        match operator {
            RedirectionOperator::Input => facts.push(ShellBehaviorFact {
                access: ShellAccessKind::Read,
                evidence: ShellBehaviorEvidence::InputRedirection,
                subject,
            }),
            RedirectionOperator::Output => facts.push(ShellBehaviorFact {
                access: ShellAccessKind::Write,
                evidence: ShellBehaviorEvidence::OutputRedirection,
                subject,
            }),
            RedirectionOperator::ReadWrite => {
                for access in [ShellAccessKind::Read, ShellAccessKind::Write] {
                    facts.push(ShellBehaviorFact {
                        access,
                        evidence: ShellBehaviorEvidence::ReadWriteRedirection,
                        subject: subject.clone(),
                    });
                }
            }
            RedirectionOperator::LiteralInput => {}
        }
    }
    facts
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RedirectionOperator {
    Input,
    Output,
    ReadWrite,
    LiteralInput,
}

fn redirection_token(word: &str) -> Option<(RedirectionOperator, Option<&str>)> {
    let token = word.trim();
    let token = token.trim_start_matches(|character: char| character.is_ascii_digit());
    for (syntax, operator) in [
        ("<<<", RedirectionOperator::LiteralInput),
        ("<<-", RedirectionOperator::LiteralInput),
        ("<<", RedirectionOperator::LiteralInput),
        ("<>", RedirectionOperator::ReadWrite),
        (">>", RedirectionOperator::Output),
        (">|", RedirectionOperator::Output),
        ("<", RedirectionOperator::Input),
        (">", RedirectionOperator::Output),
    ] {
        if let Some(subject) = token.strip_prefix(syntax) {
            return Some((operator, (!subject.is_empty()).then_some(subject)));
        }
    }
    None
}

fn is_operator(word: &str) -> bool {
    redirection_token(word).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let stages =
            crate::parse_bash_command_candidates("opaque < input.rs > output.rs <> state.db")
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
}
