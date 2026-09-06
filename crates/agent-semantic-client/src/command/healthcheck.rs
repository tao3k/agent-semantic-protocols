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
            let active_bundle =
                agent_semantic_artifacts::runtime_artifact_slots::verify_runtime_artifact_bound_bundle(
                    &agent_semantic_artifacts::RuntimeArtifactStateLayout::new(&state_home)
                        .active_slot(),
                )
                .await?;
            let active_asp = active_bundle
                .member_path("asp")
                .ok_or_else(|| "active Runtime bundle omits asp".to_owned())?;
            let active_identity =
                agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
                    &tokio::fs::read(&active_asp).await.map_err(|error| {
                        format!("read active Runtime asp {}: {error}", active_asp.display())
                    })?,
                );
            if active_identity != health.resident.runtime_binary_identity {
                return Err(
                    "Developer Runtime health identity differs from the active bound bundle"
                        .to_owned(),
                );
            }
            let current_exe = std::env::current_exe()
                .map_err(|error| format!("resolve healthcheck Runtime executable: {error}"))?;
            let current_identity =
                agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
                    &tokio::fs::read(&current_exe).await.map_err(|error| {
                        format!("read healthcheck Runtime executable {}: {error}", current_exe.display())
                    })?,
                );
            if current_identity != active_identity {
                return Err(
                    "healthcheck Runtime executable differs from the active bound bundle"
                        .to_owned(),
                );
            }
        } else {
            agent_semantic_artifacts::runtime_artifact_store::promote_active_runtime_artifact_to_healthy(
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
