// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Application boundary for typed language-facade commands.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use crate::AspClient;
use crate::projection_presentation::ProjectionPresentation;
use crate::projection_presentation::render_exact_projection_response;
use agent_semantic_client_core::LanguageId;
use agent_semantic_client_protocol::AspClientExactQueryRequest;
use agent_semantic_client_protocol::AspClientWorkspaceSearchPlaybookRequest;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientOutcome;

/// Typed language operation admitted by the shared client protocol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LanguageCommandOperation {
    WorkspaceSearchPlaybook(AspClientWorkspaceSearchPlaybookRequest),
    ExactQuery(AspClientExactQueryRequest),
}

impl LanguageCommandOperation {
    fn into_route_and_params(self) -> Result<(LanguageCommandRoute, serde_json::Value), String> {
        match self {
            Self::WorkspaceSearchPlaybook(request) => encode_operation(
                LanguageCommandRoute::Server(
                    agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD,
                ),
                request,
            ),
            Self::ExactQuery(request) => {
                encode_operation(LanguageCommandRoute::Language("query"), request)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LanguageCommandRoute {
    Server(&'static str),
    Language(&'static str),
}

fn encode_operation(
    route: LanguageCommandRoute,
    request: impl serde::Serialize,
) -> Result<(LanguageCommandRoute, serde_json::Value), String> {
    serde_json::to_value(request)
        .map(|params| (route, params))
        .map_err(|error| format!("encode typed client request: {error}"))
}

/// A language command already parsed by the thin protocol CLI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageCommandRequest {
    pub language_id: LanguageId,
    pub operation: LanguageCommandOperation,
    pub project_root: PathBuf,
    pub machine_readable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LanguageCommandResponse {
    pub route: &'static str,
    pub frame: ClientFrame,
}

impl LanguageCommandResponse {
    pub fn require_ready_payload(self) -> Result<serde_json::Value, String> {
        match self.frame {
            ClientFrame::Response {
                outcome: ClientOutcome::Ready,
                result: Some(payload),
                error: None,
                ..
            } => Ok(payload),
            ClientFrame::Response { outcome, error, .. } => Err(format!(
                "typed language command did not return Ready: route={} outcome={outcome:?} error={error:?}",
                self.route
            )),
            frame => Err(format!(
                "typed language command returned a non-response frame: route={} frame={frame:?}",
                self.route
            )),
        }
    }
}

pub type LanguageCommandDispatchFuture<'a> =
    Pin<Box<dyn Future<Output = Result<LanguageCommandResponse, String>> + Send + 'a>>;

pub trait LanguageCommandClient: Send + Sync {
    fn dispatch(&self, request: LanguageCommandRequest) -> LanguageCommandDispatchFuture<'_>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeLanguageCommandClient;

impl LanguageCommandClient for RuntimeLanguageCommandClient {
    fn dispatch(&self, request: LanguageCommandRequest) -> LanguageCommandDispatchFuture<'_> {
        Box::pin(async move {
            let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
            let (route, params) = request.operation.into_route_and_params()?;
            #[cfg(unix)]
            let client = AspClient::new_from_host_capability(&state_home, &request.project_root)?;
            #[cfg(not(unix))]
            let client = AspClient::new(&state_home, &request.project_root);
            // A language facade is a client of one content-bound Runtime
            // transaction, never of a re-derived endpoint path. Host-inherited
            // descriptors remain strict; published loopback transport must be
            // delivered by the same verified handoff before Tokio opens the
            // multiplexed ClientFrame session. When no Host descriptor was
            // inherited, the Runtime service performs the bounded activation
            // transaction; clients never observe or reconstruct an endpoint.
            let client = client.admit_runtime_workspace("Search/Query").await?;
            let (route, frame) = match route {
                LanguageCommandRoute::Server(route) => {
                    let frame = client
                        .dispatch_playbook_method(route.to_owned(), params, 0)
                        .await?;
                    (route, frame)
                }
                LanguageCommandRoute::Language(route) => {
                    let frame = client
                        .dispatch(request.language_id.as_str(), route, params)
                        .await?;
                    (route, frame)
                }
            };
            Ok(LanguageCommandResponse { route, frame })
        })
    }
}

pub type LanguageCommandFuture<'a> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;

/// Sole application boundary used by the protocol language facade.
pub trait LanguageCommandApplication: Send + Sync {
    fn execute(&self, request: LanguageCommandRequest) -> LanguageCommandFuture<'_>;
}

/// Production application backed by the shared ASP Client protocol.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeLanguageCommandApplication;

impl LanguageCommandApplication for RuntimeLanguageCommandApplication {
    fn execute(&self, request: LanguageCommandRequest) -> LanguageCommandFuture<'_> {
        Box::pin(async move {
            let machine_readable = request.machine_readable;
            let response = RuntimeLanguageCommandClient.dispatch(request).await?;
            let rendered = if response.route == "query" {
                let presentation = if machine_readable {
                    ProjectionPresentation::MachineJson
                } else {
                    ProjectionPresentation::Text
                };
                render_exact_projection_response(&response.frame, presentation)?
            } else {
                serde_json::to_string(&response.frame)
                    .map_err(|error| format!("encode route response: {error}"))?
            };
            println!("{rendered}");
            Ok(())
        })
    }
}

/// Execute one typed language command through the production application.
pub async fn execute_language_command(request: LanguageCommandRequest) -> Result<(), String> {
    RuntimeLanguageCommandApplication.execute(request).await
}
