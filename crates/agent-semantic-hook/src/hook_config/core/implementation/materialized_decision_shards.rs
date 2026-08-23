//! Materializes agent-facing decisions before matcher publication.

use super::ClientHookConfig;
use crate::{CommandDecisionShard, HookDecision, materialize_source_access_deny_message};

pub struct MaterializedDecisionShards {
    pub direct_read: Vec<(String, Vec<u8>)>,
    pub shell_read: Vec<(String, Vec<u8>)>,
    pub structured_projection: Vec<u8>,
    pub shell_command: Vec<u8>,
}

impl ClientHookConfig {
    pub fn materialized_decision_shards(&self) -> Result<MaterializedDecisionShards, String> {
        let direct_read = self
            .durable_direct_read_decision_shards()?
            .into_iter()
            .map(|(extension, _placeholder, bytes)| {
                let mut decision = HookDecision::from_compact_binary(&bytes)?;
                materialize_source_access_deny_message(&mut decision);
                Ok((extension, decision.to_compact_binary()?))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let shell_read = self
            .durable_shell_read_decision_shards()?
            .into_iter()
            .map(|(extension, _placeholder, table)| {
                let table = CommandDecisionShard::map_binary_decisions(&table, |mut decision| {
                    materialize_source_access_deny_message(&mut decision);
                    decision
                })?;
                Ok((extension, table))
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok(MaterializedDecisionShards {
            direct_read,
            shell_read,
            structured_projection: self.durable_structured_projection_decision_shard()?,
            shell_command: CommandDecisionShard::map_binary_decisions(
                &self.durable_command_profile_decision_shard()?,
                |mut decision| {
                    materialize_source_access_deny_message(&mut decision);
                    decision
                },
            )?,
        })
    }
}
