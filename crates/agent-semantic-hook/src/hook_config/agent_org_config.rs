//! Compile agent-facing Org artifact hook config from strict hook client config.

use agent_semantic_config::HookClientAgentOrgArtifactsConfig;

use crate::hook_config_agent_org::CompiledAgentOrgArtifactsConfig;

pub(crate) fn compile_agent_org_artifacts_config(
    config: Option<HookClientAgentOrgArtifactsConfig>,
) -> Result<CompiledAgentOrgArtifactsConfig, String> {
    config
        .map(CompiledAgentOrgArtifactsConfig::try_from)
        .transpose()
        .map(|compiled| compiled.unwrap_or_else(CompiledAgentOrgArtifactsConfig::disabled))
}
