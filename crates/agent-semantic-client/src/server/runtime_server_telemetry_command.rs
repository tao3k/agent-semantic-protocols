// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resident OpenTelemetry query adapter for the Runtime Server CLI.

use clap::Args;

#[derive(Debug, Args)]
pub(super) struct TelemetryQueryArgs {
    #[arg(long)]
    workspace_identity: String,
    #[arg(long)]
    surface: String,
    #[arg(long)]
    stage: String,
}

pub(super) async fn run_telemetry_query(args: TelemetryQueryArgs) -> Result<(), String> {
    let state_home = super::state_home()?;
    let query =
        agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceQuery::new(
            args.workspace_identity,
            args.surface,
            args.stage,
        );
    let receipt =
        agent_semantic_client_db::runtime_server_opentelemetry::query_runtime_performance(
            &super::runtime_server_telemetry_query_socket_path(&state_home)?,
            &query,
        )
        .await?;
    println!(
        "{}",
        serde_json::to_string(&receipt)
            .map_err(|error| format!("failed to encode telemetry query receipt: {error}"))?
    );
    Ok(())
}
