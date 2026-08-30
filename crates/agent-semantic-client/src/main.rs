#![deny(dead_code)]

//! Process entry for the public `asp` client.
//!
//! Host Hook events belong exclusively to the sibling `asp-hook` executable.

fn main() -> std::process::ExitCode {
    let daemon = std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("server"))
        && std::env::args_os().nth(2).as_deref() == Some(std::ffi::OsStr::new("daemon"));
    let no_agent_client =
        !daemon && std::env::var_os("ASP_NO_AGENT").is_some_and(|value| value == "1");
    let mut runtime_builder = if daemon {
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_daemon()
    } else if no_agent_client {
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_no_agent_client()
    } else {
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_client()
    };
    let runtime = match runtime_builder.enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!(
                "{}",
                agent_semantic_client::cli_failure::materialize_cli_failure(&format!(
                    "failed to create ASP CLI runtime: {error}"
                ))
            );
            return std::process::ExitCode::from(2);
        }
    };
    let command = runtime.block_on(async {
        if no_agent_client {
            agent_semantic_client::run_binary_from_env().await
        } else {
            let command = agent_semantic_client::run_binary_from_env();
            tokio::pin!(command);
            tokio::select! {
                result = &mut command => result,
                signal = tokio::signal::ctrl_c() => match signal {
                    Ok(()) => Err(
                        "state=cancelled reasonKind=process-interrupted signal=SIGINT".to_owned(),
                    ),
                    Err(error) => Err(format!(
                        "state=failed reasonKind=signal-listener-failed error={error}"
                    )),
                },
            }
        }
    });
    match command {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!(
                "{}",
                agent_semantic_client::cli_failure::materialize_cli_failure(&message)
            );
            std::process::ExitCode::from(2)
        }
    }
}
