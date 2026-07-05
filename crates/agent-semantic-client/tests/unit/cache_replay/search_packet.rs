use bytes::Bytes;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cache_replay::{output_with_delegation_hint_lines, render_search_packet_bytes};

static GRAPH_RENDER_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn search_packet_replay_appends_advisory_delegation_hint_line() {
    let output = frontier_output_without_hint();
    let packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "fanout": "parallel",
            "instances": "targetActions",
            "branchPrompt": "reasoning-tree",
            "stateOwner": "parent",
            "fanin": "receipt",
            "iterative": true,
            "runtimeOwner": "agent-client",
            "modelClass": "cheap",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A1.rg-query", "A2.owner-items"],
            "maxCommands": 8,
            "maxTurns": 1,
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "asp-search-subagent",
                "requiredFields": ["schema", "intent", "route", "state", "evidence", "next"]
            }
        }]
    });

    let rendered = output_with_delegation_hint_lines(output, packet.to_string().as_bytes());
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert!(rendered.contains(
        "subagentHint=profile=asp-explorer mode=resident instances=single reuse=send_input spawn=if-missing forkContext=false branchPrompt=reasoning-tree stateOwner=parent fanin=receipt iterative=true decision=advisory runtimeOwner=agent-client modelClass=cheap readOnly=true noCode=true targetActions=A1.rg-query,A2.owner-items maxCommands=8 maxTurns=1 receipt=asp-search-subagent(schema,intent,route,state,evidence,next) reason=query-selector-low-confidence"
    ));
}

#[test]
fn search_packet_replay_canonicalizes_existing_hint_line() {
    let output = Bytes::from(format!(
        "{}subagentHint=profile=asp-explorer fanout=parallel instances=targetActions branchPrompt=reasoning-tree stateOwner=parent fanin=receipt iterative=true decision=advisory runtimeOwner=agent-client modelClass=cheap readOnly=true noCode=true targetActions=A1.rg-query maxCommands=8 maxTurns=1 receipt=asp-search-subagent(role,action,evidence,missing,next,risk) reason=query-selector-low-confidence\n",
        std::str::from_utf8(&frontier_output_without_hint()).expect("utf8 output")
    ));
    let packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "runtimeOwner": "agent-client",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A2.owner-items"],
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "asp-search-subagent",
                "requiredFields": ["role", "action", "evidence"]
            }
        }]
    });

    let rendered = output_with_delegation_hint_lines(output, packet.to_string().as_bytes());
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert_eq!(rendered.matches("subagentHint=").count(), 1);
    assert!(rendered.contains("mode=resident instances=single reuse=send_input"));
    assert!(rendered.contains("targetActions=A2.owner-items"));
    assert!(!rendered.contains("fanout=parallel"));
}

#[test]
fn search_packet_replay_ignores_non_client_delegation_hints() {
    let output = frontier_output_without_hint();
    let packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "runtimeOwner": "provider",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A1.rg-query"],
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "asp-search-subagent",
                "requiredFields": ["role"]
            }
        }]
    });

    let rendered = output_with_delegation_hint_lines(output, packet.to_string().as_bytes());
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert!(!rendered.contains("subagentHint="));
}

#[test]
fn search_packet_replay_rejects_invalid_hint_limits() {
    let output = frontier_output_without_hint();
    let packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "runtimeOwner": "agent-client",
            "modelClass": "expensive-model",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A1.rg-query"],
            "maxCommands": 0,
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "asp-search-subagent",
                "requiredFields": ["role"]
            }
        }]
    });

    let rendered = output_with_delegation_hint_lines(output, packet.to_string().as_bytes());
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert!(!rendered.contains("subagentHint="));
}

#[test]
fn search_packet_replay_appends_delegation_hint_after_graph_render() {
    let _guard = GRAPH_RENDER_ENV_LOCK.lock().expect("graph render env lock");
    let root = temp_root("graph-render-delegation-hint");
    let renderer = write_fake_graph_renderer(&root);
    let _env = GraphRendererEnvGuard::set(&renderer);

    let packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "runtimeOwner": "agent-client",
            "modelClass": "cheap",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A1.rg-query"],
            "maxCommands": 4,
            "maxTurns": 1,
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "asp-search-subagent",
                "requiredFields": ["role", "evidence"]
            }
        }]
    });

    let rendered =
        render_search_packet_bytes(Bytes::from(packet.to_string())).expect("rendered packet");
    let _ = std::fs::remove_dir_all(root);
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert!(rendered.starts_with("[search-pipe]"));
    assert!(rendered.contains("subagentHint=profile=asp-explorer"));
    assert!(rendered.contains("targetActions=A1.rg-query"));
    assert!(rendered.contains("maxCommands=4"));
}

#[test]
fn search_packet_replay_accepts_colon_alias_graph_lines() {
    let _guard = GRAPH_RENDER_ENV_LOCK.lock().expect("graph render env lock");
    let root = temp_root("graph-render-colon-alias");
    let renderer = write_fake_colon_alias_graph_renderer(&root);
    let _env = GraphRendererEnvGuard::set(&renderer);

    let packet = json!({"view": "dependency"});

    let rendered =
        render_search_packet_bytes(Bytes::from(packet.to_string())).expect("rendered packet");
    let _ = std::fs::remove_dir_all(root);
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert!(rendered.starts_with("[search-dependency]"));
    assert!(rendered.contains("aliases: graph:{G=search,D=dependency}"));
}

fn frontier_output_without_hint() -> Bytes {
    Bytes::from_static(
        b"[search-pipe] q=delegation view=seeds alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases=G:search,Q:query\n\
Q=query:term(delegation)!query\n\
G>{Q:matches}\n\
rank=Q frontier=Q.query\n",
    )
}

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn write_fake_colon_alias_graph_renderer(root: &Path) -> PathBuf {
    let renderer = root.join("fake-colon-alias-graph-renderer.sh");
    std::fs::write(
        &renderer,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '[search-dependency] q=tokio alg=seed-frontier'\nprintf '%s\\n' 'legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next'\nprintf '%s\\n' 'aliases: graph:{G=search,D=dependency}'\nprintf '%s\\n' 'D=dependency:pkg(tokio)!dependency'\nprintf '%s\\n' 'G>{D:uses}'\nprintf '%s\\n' 'rank=D frontier=D.dependency'\n",
    )
    .expect("write fake graph renderer");
    make_executable(&renderer);
    renderer
}

fn write_fake_graph_renderer(root: &Path) -> PathBuf {
    let renderer = root.join("fake-graph-renderer.sh");
    std::fs::write(
        &renderer,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '[search-pipe] q=delegation view=seeds alg=seed-frontier'\nprintf '%s\\n' 'legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next'\nprintf '%s\\n' 'aliases=G:search,Q:query'\nprintf '%s\\n' 'Q=query:term(delegation)!query'\nprintf '%s\\n' 'G>{Q:matches}'\nprintf '%s\\n' 'rank=Q frontier=Q.query'\n",
    )
    .expect("write fake graph renderer");
    make_executable(&renderer);
    renderer
}

struct GraphRendererEnvGuard {
    previous: Option<std::ffi::OsString>,
}

impl GraphRendererEnvGuard {
    fn set(renderer: &Path) -> Self {
        let previous = std::env::var_os("SEMANTIC_AGENT_PROTOCOL_BIN");
        unsafe {
            std::env::set_var("SEMANTIC_AGENT_PROTOCOL_BIN", renderer);
        }
        Self { previous }
    }
}

impl Drop for GraphRendererEnvGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(previous) = &self.previous {
                std::env::set_var("SEMANTIC_AGENT_PROTOCOL_BIN", previous);
            } else {
                std::env::remove_var("SEMANTIC_AGENT_PROTOCOL_BIN");
            }
        }
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .expect("fake graph renderer metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("fake graph renderer permissions");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
