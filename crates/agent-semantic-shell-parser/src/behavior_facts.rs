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
    words
        .iter()
        .enumerate()
        .flat_map(|(index, word)| behavior_facts_for_word(words, index, word))
        .collect()
}

fn behavior_facts_for_word(words: &[String], index: usize, word: &str) -> Vec<ShellBehaviorFact> {
    let Some((operator, inline_subject)) = redirection_token(word) else {
        return Vec::new();
    };
    let subject = inline_subject.map(str::to_owned).or_else(|| {
        words
            .get(index + 1)
            .filter(|next| !is_operator(next))
            .cloned()
    });
    match operator {
        RedirectionOperator::Input => vec![ShellBehaviorFact {
            access: ShellAccessKind::Read,
            evidence: ShellBehaviorEvidence::InputRedirection,
            subject,
        }],
        RedirectionOperator::Output => vec![ShellBehaviorFact {
            access: ShellAccessKind::Write,
            evidence: ShellBehaviorEvidence::OutputRedirection,
            subject,
        }],
        RedirectionOperator::ReadWrite => [ShellAccessKind::Read, ShellAccessKind::Write]
            .into_iter()
            .map(|access| ShellBehaviorFact {
                access,
                evidence: ShellBehaviorEvidence::ReadWriteRedirection,
                subject: subject.clone(),
            })
            .collect(),
        RedirectionOperator::LiteralInput => Vec::new(),
    }
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
#[path = "../tests/unit/behavior_facts.rs"]
mod tests;
