//! Multi-Agent v2 session registration commands.
//!
//! The parent id is carried as an explicit command argument in the native
//! Collaboration message. The child id is owned by Codex and is read only
//! from the child process environment. Natural-language prose is never parsed
//! as identity evidence.

use agent_semantic_client_protocol::AGENT_SESSION_REGISTER_METHOD;
use agent_semantic_client_protocol::AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID;
use agent_semantic_client_protocol::AgentChildThreadId;
use agent_semantic_client_protocol::AgentName;
use agent_semantic_client_protocol::AgentParentThreadId;
use agent_semantic_client_protocol::AgentPath;
use agent_semantic_client_protocol::AgentRootSessionId;
use agent_semantic_client_protocol::AgentRouteKey;
use agent_semantic_client_protocol::AgentSessionRegisterReceipt;
use agent_semantic_client_protocol::AgentSessionRegisterRequest;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientOutcome;
use agent_semantic_client_protocol::ClientSchemaId;

pub(super) async fn run_session_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("register-child") => register_child(parse_register_child_args(&args[1..])?).await,
        Some("help" | "--help" | "-h") | None => {
            println!("{}", session_usage());
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown asp session command `{command}`\n{}",
            session_usage()
        )),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct RegisterChildArgs {
    parent_thread_id: String,
    agent_name: String,
}

fn parse_register_child_args(args: &[String]) -> Result<RegisterChildArgs, String> {
    let mut parent_thread_id = None;
    let mut agent_name = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--parent-thread-id" => {
                index += 1;
                parent_thread_id = args.get(index).cloned();
            }
            "--agent-name" => {
                index += 1;
                agent_name = args.get(index).cloned();
            }
            option => {
                return Err(format!(
                    "unknown asp session register-child option `{option}`\n{}",
                    session_usage()
                ));
            }
        }
        index += 1;
    }
    let parent_thread_id = required_non_empty(parent_thread_id, "--parent-thread-id")?;
    let agent_name = required_non_empty(agent_name, "--agent-name")?;
    Ok(RegisterChildArgs {
        parent_thread_id,
        agent_name,
    })
}

fn required_non_empty(value: Option<String>, option: &str) -> Result<String, String> {
    required_non_empty_for(value, "register-child", option)
}

fn required_non_empty_for(
    value: Option<String>,
    command: &str,
    option: &str,
) -> Result<String, String> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("asp session {command} requires {option}"))
}

async fn register_child(args: RegisterChildArgs) -> Result<(), String> {
    let mut ready =
        crate::server::runtime_server::ensure_healthy_runtime_server_for_bounded_operation()
            .await?;
    let transaction = ready.resident_transaction.take().ok_or_else(|| {
        "reasonKind=runtime-client-handoff-unavailable failureLayer=runtime-resident-transaction Runtime bootstrap returned Healthy without its resident transaction"
            .to_owned()
    })?;
    let handoff = crate::AspClientRuntimeHandoff::try_from(&transaction)?;
    let binding = current_codex_thread_binding(&args.parent_thread_id)?;

    register_session_binding(binding, args.agent_name, handoff).await
}

#[derive(Debug, Eq, PartialEq)]
struct CodexThreadBinding {
    root_session_id: String,
    parent_thread_id: String,
    child_thread_id: String,
}

async fn register_session_binding(
    binding: CodexThreadBinding,
    agent_name: String,
    handoff: crate::AspClientRuntimeHandoff,
) -> Result<(), String> {
    let state_home = agent_semantic_runtime::resolve_state_home()?;
    let registry_path = state_home.join("agents/config.toml");
    let agents =
        agent_semantic_config::agent_route_registry::load_agent_route_registry_for_platform(
            &registry_path,
            "codex",
        )?;
    let route = agents
        .compile_route_for_platform_host_agent_name("codex", &agent_name)?
        .ok_or_else(|| {
            format!(
                "Codex child registration requires an AgentLoader-owned name; `{}` is not registered in {}",
                agent_name,
                registry_path.display()
            )
        })?;
    if !route.is_resident_agent() {
        return Err(format!(
            "Codex child registration requires a resident configured Agent; `{}` is agentKind={} lifetime={}",
            agent_name,
            route.agent_kind,
            route.session_lifetime.as_str()
        ));
    }

    let cwd = std::env::current_dir()
        .map_err(|error| format!("failed to resolve current workspace: {error}"))?;
    let route_name = route.route_key.as_str();
    let agent_path = format!("/root/{agent_name}");
    let request = AgentSessionRegisterRequest {
        schema_id: ClientSchemaId::new(AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID)?,
        schema_version: 1,
        root_session_id: AgentRootSessionId::new(binding.root_session_id)?,
        parent_thread_id: AgentParentThreadId::new(binding.parent_thread_id)?,
        child_thread_id: AgentChildThreadId::new(binding.child_thread_id)?,
        agent_name: AgentName::new(agent_name)?,
        agent_path: AgentPath::new(agent_path)?,
        route_key: AgentRouteKey::new(route_name)?,
    };
    request.validate()?;
    let params = serde_json::to_value(&request)
        .map_err(|error| format!("encode child registration request: {error}"))?;
    let frame = crate::runtime_language_client::AspClient::new_from_runtime_handoff(
        state_home, cwd, handoff,
    )
    .dispatch_method(AGENT_SESSION_REGISTER_METHOD.to_owned(), params)
    .await?;
    let receipt: AgentSessionRegisterReceipt = match frame {
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            result: Some(result),
            error: None,
            ..
        } => serde_json::from_value(result)
            .map_err(|error| format!("decode child registration receipt: {error}"))?,
        ClientFrame::Response { outcome, error, .. } => {
            return Err(format!(
                "child registration dispatch failed: outcome={outcome:?} error={}",
                error.unwrap_or(serde_json::Value::Null)
            ));
        }
        frame => {
            return Err(format!(
                "child registration dispatch returned a non-response frame: {frame:?}"
            ));
        }
    };
    receipt.validate()?;
    println!(
        "{}",
        serde_json::to_string(&receipt)
            .map_err(|error| format!("encode child registration receipt: {error}"))?
    );
    Ok(())
}

fn current_codex_thread_binding(parent_thread_id: &str) -> Result<CodexThreadBinding, String> {
    resolve_codex_thread_binding(
        parent_thread_id,
        std::env::var("CODEX_THREAD_ID").ok(),
        std::env::var("CODEX_SESSION_ID").ok(),
    )
}

fn resolve_codex_thread_binding(
    parent_thread_id: &str,
    thread_id: Option<String>,
    session_id: Option<String>,
) -> Result<CodexThreadBinding, String> {
    let thread_id = thread_id.filter(|value| !value.trim().is_empty());
    let session_id = session_id.filter(|value| !value.trim().is_empty());
    let thread_id = thread_id.ok_or_else(|| {
        "asp session register-child must run inside the delivered Codex child; CODEX_THREAD_ID is missing"
            .to_owned()
    })?;
    let session_id = session_id.ok_or_else(|| {
        "asp session register-child requires the Codex root-session binding; CODEX_SESSION_ID is missing"
            .to_owned()
    })?;
    if thread_id == parent_thread_id {
        return Err(
            "child registration requires distinct parentThreadId and child CODEX_THREAD_ID"
                .to_owned(),
        );
    }
    Ok(CodexThreadBinding {
        root_session_id: session_id,
        parent_thread_id: parent_thread_id.to_owned(),
        child_thread_id: thread_id,
    })
}

fn session_usage() -> &'static str {
    "usage: asp session register-child --parent-thread-id <parent-thread> --agent-name <configured-agent>"
}

#[cfg(test)]
#[path = "../../tests/unit/command/session.rs"]
mod tests;
