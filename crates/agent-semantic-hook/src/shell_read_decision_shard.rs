//! Compact config-compiled decision table selected by normalized argv prefix.

use serde::{Deserialize, Serialize};

use crate::HookDecision;

#[derive(Serialize, Deserialize)]
struct ShellReadDecisionEntry {
    argv_prefix: Vec<String>,
    match_kind: CommandDecisionMatchKind,
    decision: Vec<u8>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum CommandDecisionMatchKind {
    Prefix,
    ProcessEnvironmentAssignment,
}

/// Immutable decision table selected by normalized argv prefix.
#[derive(Serialize, Deserialize)]
pub struct CommandDecisionShard {
    entries: Vec<ShellReadDecisionEntry>,
}

impl CommandDecisionShard {
    /// Build a table from config-compiled winning decisions.
    pub fn new(entries: Vec<(Vec<String>, HookDecision)>) -> Result<Self, String> {
        Self::new_with_process_environment_assignments(entries, Vec::new())
    }

    pub fn new_with_process_environment_assignments(
        prefix_entries: Vec<(Vec<String>, HookDecision)>,
        environment_entries: Vec<(Vec<String>, HookDecision)>,
    ) -> Result<Self, String> {
        prefix_entries
            .into_iter()
            .map(|entry| (entry, CommandDecisionMatchKind::Prefix))
            .chain(environment_entries.into_iter().map(|entry| {
                (
                    entry,
                    CommandDecisionMatchKind::ProcessEnvironmentAssignment,
                )
            }))
            .map(|((argv_prefix, decision), match_kind)| {
                Ok(ShellReadDecisionEntry {
                    argv_prefix,
                    match_kind,
                    decision: decision.to_compact_binary()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()
            .map(|entries| Self { entries })
    }

    fn select_entry<'a>(
        &'a self,
        command_tokens: &[String],
        command_stages: Option<&[agent_semantic_shell_parser::CommandStage]>,
    ) -> Option<&'a ShellReadDecisionEntry> {
        if let Some(stages) = command_stages
            && let Some(selected) = self
                .entries
                .iter()
                .filter(|entry| {
                    matches!(
                        entry.match_kind,
                        CommandDecisionMatchKind::ProcessEnvironmentAssignment
                    ) && agent_semantic_shell_parser::command_stages_match_process_environment_assignment(
                        stages,
                        &entry.argv_prefix,
                    )
                })
                .max_by_key(|entry| entry.argv_prefix.len())
        {
            return Some(selected);
        }
        self.entries
            .iter()
            .filter(|entry| matches!(entry.match_kind, CommandDecisionMatchKind::Prefix))
            .filter(|entry| {
                !entry.argv_prefix.is_empty()
                    && command_tokens
                        .windows(entry.argv_prefix.len())
                        .any(|candidate| {
                            agent_semantic_shell_parser::candidate_matches_prefix(
                                candidate,
                                &entry.argv_prefix,
                            )
                        })
            })
            .max_by_key(|entry| entry.argv_prefix.len())
    }

    /// Encode the compact table for atomic mmap publication.
    pub fn to_binary_bytes(&self) -> Result<Vec<u8>, String> {
        postcard::to_allocvec(self)
            .map_err(|error| format!("encode shell-read decision shard: {error}"))
    }

    /// Decode and select the winning decision for normalized command tokens.
    pub fn select(bytes: &[u8], command_tokens: &[String]) -> Result<Option<HookDecision>, String> {
        let shard = postcard::from_bytes::<Self>(bytes)
            .map_err(|error| format!("decode shell-read decision shard: {error}"))?;
        let selected = shard.select_entry(command_tokens, None);
        selected
            .map(|entry| HookDecision::from_compact_binary(&entry.decision))
            .transpose()
    }

    pub fn select_for_command(
        bytes: &[u8],
        command: &str,
        command_tokens: &[String],
    ) -> Result<Option<HookDecision>, String> {
        let shard = postcard::from_bytes::<Self>(bytes)
            .map_err(|error| format!("decode shell-read decision shard: {error}"))?;
        // Prefix-only commands are the overwhelmingly common Hook path.  A
        // A process environment assignment cannot exist without `=`, so avoid
        // constructing the parser-owned command graph unless an environment
        // matcher can possibly win.  The lexical check is only a negative
        // performance gate; every positive match still belongs to the parser.
        let has_environment_matcher = shard.entries.iter().any(|entry| {
            matches!(
                entry.match_kind,
                CommandDecisionMatchKind::ProcessEnvironmentAssignment
            )
        });
        let stages = (has_environment_matcher && command.contains('='))
            .then(|| {
                agent_semantic_shell_parser::parse_bash_command_candidates(command)
                    .map_err(|error| format!("parse shell command decision key: {error}"))
            })
            .transpose()?;
        shard
            .select_entry(command_tokens, stages.as_deref())
            .map(|entry| HookDecision::from_compact_binary(&entry.decision))
            .transpose()
    }

    /// Select only a declarative process-environment decision. This terminal
    /// layer is evaluated before specialized structured-projector routing.
    pub fn select_process_environment_for_command(
        bytes: &[u8],
        command: &str,
    ) -> Result<Option<HookDecision>, String> {
        if !command.contains('=') {
            return Ok(None);
        }
        let shard = postcard::from_bytes::<Self>(bytes)
            .map_err(|error| format!("decode command decision shard: {error}"))?;
        let stages = agent_semantic_shell_parser::parse_bash_command_candidates(command)
            .map_err(|error| format!("parse process environment decision key: {error}"))?;
        shard
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.match_kind,
                    CommandDecisionMatchKind::ProcessEnvironmentAssignment
                ) && agent_semantic_shell_parser::command_stages_match_process_environment_assignment(
                    &stages,
                    &entry.argv_prefix,
                )
            })
            .max_by_key(|entry| entry.argv_prefix.len())
            .map(|entry| HookDecision::from_compact_binary(&entry.decision))
            .transpose()
    }

    /// Rewrite every precompiled decision while preserving the config-derived
    /// argv table. Control-plane publication uses this to materialize static
    /// agent guidance once instead of on every Hook process.
    pub fn map_binary_decisions(
        bytes: &[u8],
        mut map: impl FnMut(HookDecision) -> HookDecision,
    ) -> Result<Vec<u8>, String> {
        let mut shard = postcard::from_bytes::<Self>(bytes)
            .map_err(|error| format!("decode command decision shard: {error}"))?;
        for entry in &mut shard.entries {
            let decision = HookDecision::from_compact_binary(&entry.decision)?;
            entry.decision = map(decision).to_compact_binary()?;
        }
        shard.to_binary_bytes()
    }
}

#[cfg(test)]
#[path = "../tests/unit/shell_read_decision_shard.rs"]
mod tests;
