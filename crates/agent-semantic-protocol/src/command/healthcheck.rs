#[derive(Debug)]
struct HealthcheckOptions {
    json: bool,
}

impl HealthcheckOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut json = false;
        for arg in args {
            match arg.as_str() {
                "--json" => json = true,
                "--help" | "-h" => return Err(usage()),
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown asp healthcheck option {arg}\n{}", usage()));
                }
                _ => {
                    return Err(format!(
                        "asp healthcheck is Global and accepts no PROJECT_ROOT\n{}",
                        usage()
                    ));
                }
            }
        }
        Ok(Self { json })
    }
}

fn usage() -> String {
    "usage: asp healthcheck [--json]".to_owned()
}

pub(super) async fn run_healthcheck_command(args: &[String]) -> Result<(), String> {
    let options = HealthcheckOptions::parse(args)?;
    let state_home = crate::server::runtime_server::state_home()?;
    crate::server::runtime_server::ensure_runtime_server_for_healthcheck(&state_home).await?;
    let health = agent_semantic_client_db::runtime_server_health::cached_runtime_server_health_at(
        &agent_semantic_client_db::runtime_server_control::runtime_server_runtime_base(
            &state_home,
        )?,
    )
    .await?;
    if health.is_healthy() {
        agent_semantic_artifacts::runtime_artifact_catalog::promote_active_runtime_artifact_to_healthy(
            &state_home,
            "asp",
            &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
                &format!(
                    "blake3-256:{}",
                    health.resident.runtime_binary_identity.value()
                ),
            )?,
        )
        .await?;
    }
    if options.json {
        println!(
            "{}",
            serde_json::to_string(&health).map_err(|error| {
                format!("failed to encode Runtime Server cached health: {error}")
            })?
        );
    } else {
        println!(
            "[asp-healthcheck] status={} source=resident-status-memory elapsedMicros={} artifact={} transport={} workspaces={}",
            if health.is_healthy() {
                "ok"
            } else {
                "degraded"
            },
            health.elapsed_micros,
            health.resident.runtime_binary_identity.value(),
            health.resident.transport_contract_digest,
            health.resident.workspace_entry_count,
        );
    }
    if health.is_healthy() {
        Ok(())
    } else {
        Err(health
            .resident
            .reason
            .unwrap_or_else(|| "Runtime Server cached health is not healthy".to_owned()))
    }
}
