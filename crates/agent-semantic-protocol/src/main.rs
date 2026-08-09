#![deny(dead_code)]

fn main() -> std::process::ExitCode {
    let hook_args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let hook_dispatch =
        agent_semantic_protocol::hook_bootstrap::is_hook_event_dispatch(hook_args.clone());
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
    } else {
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_cli()
    };
    let runtime = match runtime_builder.enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to create ASP CLI runtime: {error}");
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
            ProcessOutcome::Command(agent_semantic_protocol::run_binary_from_env().await)
        }
    });
    match outcome {
        ProcessOutcome::Hook(code) => {
            agent_semantic_protocol::hook_bootstrap::terminate_hook_process(code)
        }
        ProcessOutcome::Command(Ok(())) => std::process::ExitCode::SUCCESS,
        ProcessOutcome::Command(Err(message)) => {
            eprintln!("{message}");
            std::process::ExitCode::from(2)
        }
    }
}
