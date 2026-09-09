// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime Server-owned Hook health projection.

use std::path::PathBuf;

const CODEX_MULTI_AGENT_V2_REASON_KIND: &str = "codex-multi-agent-v2-capability-unavailable";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CodexMultiAgentV2Status {
    Enabled,
    ConfigMissing,
    ConfigInvalid,
    FeatureMissing,
    FeatureDisabled,
}

impl CodexMultiAgentV2Status {
    fn label(self) -> &'static str {
        match self {
            Self::Enabled => "ok",
            Self::ConfigMissing
            | Self::ConfigInvalid
            | Self::FeatureMissing
            | Self::FeatureDisabled => "degraded",
        }
    }

    fn reason_kind(self) -> &'static str {
        match self {
            Self::Enabled => "none",
            Self::ConfigMissing => "codex-config-missing",
            Self::ConfigInvalid => "codex-config-invalid",
            Self::FeatureMissing | Self::FeatureDisabled => CODEX_MULTI_AGENT_V2_REASON_KIND,
        }
    }
}

fn codex_config_path() -> Result<PathBuf, String> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(codex_home).join("config.toml"));
    }
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".codex").join("config.toml"))
        .ok_or_else(|| "CODEX_HOME and HOME are both unavailable".to_owned())
}

fn codex_multi_agent_v2_status(input: Option<&str>) -> CodexMultiAgentV2Status {
    let Some(input) = input else {
        return CodexMultiAgentV2Status::ConfigMissing;
    };
    let Ok(config) = toml::from_str::<toml::Value>(input) else {
        return CodexMultiAgentV2Status::ConfigInvalid;
    };
    match config
        .get("features")
        .and_then(|features| features.get("multi_agent_v2"))
        .and_then(toml::Value::as_bool)
    {
        Some(true) => CodexMultiAgentV2Status::Enabled,
        Some(false) => CodexMultiAgentV2Status::FeatureDisabled,
        None => CodexMultiAgentV2Status::FeatureMissing,
    }
}

fn report_codex_multi_agent_v2() -> Result<(), String> {
    let config_path = codex_config_path()?;
    let input = match std::fs::read_to_string(&config_path) {
        Ok(input) => Some(input),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "failed to read Codex config {}: {error}",
                config_path.display()
            ));
        }
    };
    let status = codex_multi_agent_v2_status(input.as_deref());
    println!(
        "[hook-doctor] status={} authority=codex-host-config configPath={} feature=multi_agent_v2 enabled={} reasonKind={} required='[features] multi_agent_v2 = true'",
        status.label(),
        config_path.display(),
        status == CodexMultiAgentV2Status::Enabled,
        status.reason_kind(),
    );
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_doctor.rs"]
mod multi_agent_v2_tests;

pub(super) async fn run_doctor(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: asp hook doctor".to_owned());
    }
    report_codex_multi_agent_v2()?;
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let health = agent_semantic_client_db::runtime_server_health::
        cached_runtime_server_health_for_state_home(&state_home).await?;
    println!(
        "[hook-doctor] status={} authority=runtime-server state={:?} elapsedMicros={}",
        if health.is_healthy() {
            "ok"
        } else {
            "degraded"
        },
        health.resident.state,
        health.elapsed_micros,
    );
    Ok(())
}
