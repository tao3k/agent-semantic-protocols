#![deny(dead_code)]

fn main() -> std::process::ExitCode {
    // Keep `asp hook` as the stable public ABI while the bootstrap remains a
    // thin async adapter on the same Tokio lifecycle as every other command.
    let hook_dispatch = agent_semantic_protocol::hook_bootstrap::is_hook_event_dispatch(
        std::env::args_os().skip(1),
    );
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
