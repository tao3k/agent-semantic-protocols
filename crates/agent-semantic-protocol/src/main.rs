#![deny(dead_code)]

fn main() -> std::process::ExitCode {
    let hook_args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let hook_dispatch =
        agent_semantic_protocol::hook_bootstrap::is_hook_event_dispatch(hook_args.clone());
    if hook_dispatch && std::env::var_os("ASP_HOOK_BOOTSTRAP_TRACE").is_some() {
        eprintln!("[asp-hook] route=process-entry-hook-dispatch");
    }
    if let Some(result) =
        agent_semantic_protocol::hook_bootstrap::process_entry_no_agent_bypass(&hook_args)
    {
        let code = match result {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("[asp-hook] status=failed error={error}");
                2
            }
        };
        agent_semantic_protocol::hook_bootstrap::terminate_hook_process(code)
    }
    let synchronous_hook_dispatch =
        agent_semantic_protocol::hook_bootstrap::is_synchronous_hook_dispatch(hook_args);
    if synchronous_hook_dispatch {
        if std::env::var_os("ASP_HOOK_BOOTSTRAP_TRACE").is_some() {
            eprintln!("[asp-hook] route=process-entry-synchronous-policy-data-plane");
        }
        let code =
            agent_semantic_protocol::hook_bootstrap::run_synchronous_hook_bootstrap_from_env();
        agent_semantic_protocol::hook_bootstrap::terminate_hook_process(code)
    }
    if hook_dispatch && std::env::var_os("ASP_HOOK_BOOTSTRAP_TRACE").is_some() {
        eprintln!("[asp-hook] route=process-entry-async-lifecycle-runtime-build");
    }
    let daemon = std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("server"))
        && std::env::args_os().nth(2).as_deref() == Some(std::ffi::OsStr::new("daemon"));
    let mut runtime_builder = if daemon {
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_daemon()
    } else if hook_dispatch {
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_hook_client()
    } else {
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_client()
    };
    let runtime = match runtime_builder.enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!(
                "{}",
                agent_semantic_protocol::cli_failure::materialize_cli_failure(&format!(
                    "failed to create ASP CLI runtime: {error}"
                ))
            );
            return std::process::ExitCode::from(2);
        }
    };
    if hook_dispatch && std::env::var_os("ASP_HOOK_BOOTSTRAP_TRACE").is_some() {
        eprintln!("[asp-hook] route=process-entry-async-lifecycle-runtime-ready");
    }
    enum ProcessOutcome {
        Hook(i32),
        Command(Result<(), String>),
    }
    let outcome = runtime.block_on(async {
        if hook_dispatch {
            ProcessOutcome::Hook(
                agent_semantic_protocol::hook_bootstrap::run_hook_bootstrap_from_env().await,
            )
        } else {
            let command = agent_semantic_protocol::run_binary_from_env();
            tokio::pin!(command);
            ProcessOutcome::Command(tokio::select! {
                result = &mut command => result,
                signal = tokio::signal::ctrl_c() => match signal {
                    Ok(()) => Err(
                        "state=cancelled reasonKind=process-interrupted signal=SIGINT".to_owned(),
                    ),
                    Err(error) => Err(format!(
                        "state=failed reasonKind=signal-listener-failed error={error}"
                    )),
                },
            })
        }
    });
    match outcome {
        ProcessOutcome::Hook(code) => {
            agent_semantic_protocol::hook_bootstrap::terminate_hook_process(code)
        }
        ProcessOutcome::Command(Ok(())) => std::process::ExitCode::SUCCESS,
        ProcessOutcome::Command(Err(message)) => {
            eprintln!(
                "{}",
                agent_semantic_protocol::cli_failure::materialize_cli_failure(&message)
            );
            std::process::ExitCode::from(2)
        }
    }
}
