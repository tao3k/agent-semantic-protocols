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
    let health = agent_semantic_client_db::runtime_server_health::
        cached_runtime_server_health_for_state_home(&state_home).await?;
    if health.is_healthy() {
        if agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_developer_root(
            &state_home,
        )?
        .is_some()
        {
            let identity =
                agent_semantic_runtime::runtime_artifact_identity::read_runtime_artifact_identity(
                    &state_home,
                    "asp",
                )
                .await?;
            if identity.identity()? != health.resident.runtime_binary_identity {
                return Err(
                    "Developer Runtime health identity differs from the direct-link publication"
                        .to_owned(),
                );
            }
            agent_semantic_runtime::runtime_artifact_identity::admit_runtime_invoker(
                &std::env::current_exe()
                    .map_err(|error| format!("resolve healthcheck Runtime executable: {error}"))?,
                &identity,
                &state_home.join("runtime/bin/asp"),
            )?;
        } else {
            agent_semantic_artifacts::runtime_artifact_catalog::promote_active_runtime_artifact_to_healthy(
                &state_home,
                "asp",
                health.resident.runtime_binary_identity.content_digest(),
            )
            .await?;
        }
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
            health
                .resident
                .runtime_binary_identity
                .content_digest()
                .as_str(),
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
