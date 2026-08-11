//! Compact config-compiled decision table selected by normalized argv prefix.

use serde::{Deserialize, Serialize};

use crate::HookDecision;

#[derive(Serialize, Deserialize)]
struct ShellReadDecisionEntry {
    argv_prefix: Vec<String>,
    decision: Vec<u8>,
}

/// Immutable decision table selected by normalized argv prefix.
#[derive(Serialize, Deserialize)]
pub struct CommandDecisionShard {
    entries: Vec<ShellReadDecisionEntry>,
}

impl CommandDecisionShard {
    /// Build a table from config-compiled winning decisions.
    pub fn new(entries: Vec<(Vec<String>, HookDecision)>) -> Result<Self, String> {
        entries
            .into_iter()
            .map(|(argv_prefix, decision)| {
                Ok(ShellReadDecisionEntry {
                    argv_prefix,
                    decision: decision.to_compact_binary()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()
            .map(|entries| Self { entries })
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
        let selected = shard
            .entries
            .iter()
            .filter(|entry| {
                !entry.argv_prefix.is_empty()
                    && command_tokens
                        .windows(entry.argv_prefix.len())
                        .any(|candidate| {
                            agent_semantic_command_match::candidate_matches_prefix(
                                candidate,
                                &entry.argv_prefix,
                            )
                        })
            })
            .max_by_key(|entry| entry.argv_prefix.len());
        selected
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
