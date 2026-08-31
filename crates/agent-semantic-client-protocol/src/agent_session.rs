use serde::{Deserialize, Serialize};

pub const AGENT_SESSION_REGISTER_METHOD: &str = "asp.session.register-child";
pub const AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.codex-child-session-registration-request";
pub const AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.codex-child-session-registration-receipt";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionRegisterRequest {
    pub schema_id: String,
    pub schema_version: u64,
    pub root_session_id: String,
    pub parent_thread_id: String,
    pub child_thread_id: String,
    pub agent_name: String,
    pub agent_path: String,
    pub route_key: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionRegisterReceipt {
    pub schema_id: String,
    pub schema_version: u64,
    pub state: String,
    pub platform: String,
    pub project_id: String,
    pub root_session_id: String,
    pub parent_thread_id: String,
    pub child_thread_id: String,
    pub agent_name: String,
    pub agent_path: String,
    pub route_key: String,
    pub physical_generation: u64,
    pub registry_owner: String,
    pub transport: String,
}

impl AgentSessionRegisterRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID || self.schema_version != 1 {
            return Err("child registration request schema identity is invalid".to_owned());
        }
        for (field, value) in [
            ("rootSessionId", self.root_session_id.as_str()),
            ("parentThreadId", self.parent_thread_id.as_str()),
            ("childThreadId", self.child_thread_id.as_str()),
            ("agentName", self.agent_name.as_str()),
            ("agentPath", self.agent_path.as_str()),
            ("routeKey", self.route_key.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("child registration {field} must be non-empty"));
            }
        }
        if self.parent_thread_id == self.child_thread_id {
            return Err(
                "child registration requires distinct parent and child thread ids".to_owned(),
            );
        }
        if self.agent_path != format!("/root/{}", self.agent_name) {
            return Err("child registration agentPath does not match agentName".to_owned());
        }
        Ok(())
    }
}

impl AgentSessionRegisterReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID
            || self.schema_version != 1
            || self.state != "registered"
            || self.platform != "codex"
            || self.registry_owner != "runtime-server-agent-session-registry"
            || self.transport != "grpc-client-frame"
            || self.physical_generation == 0
        {
            return Err("child registration receipt is not current".to_owned());
        }
        AgentSessionRegisterRequest {
            schema_id: AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
            schema_version: 1,
            root_session_id: self.root_session_id.clone(),
            parent_thread_id: self.parent_thread_id.clone(),
            child_thread_id: self.child_thread_id.clone(),
            agent_name: self.agent_name.clone(),
            agent_path: self.agent_path.clone(),
            route_key: self.route_key.clone(),
        }
        .validate()?;
        if self.project_id.trim().is_empty() {
            return Err("child registration receipt projectId must be non-empty".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_request_binds_distinct_parent_child_and_canonical_path() {
        let request = AgentSessionRegisterRequest {
            schema_id: AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
            schema_version: 1,
            root_session_id: "root-1".to_owned(),
            parent_thread_id: "parent-1".to_owned(),
            child_thread_id: "child-1".to_owned(),
            agent_name: "asp_testing".to_owned(),
            agent_path: "/root/asp_testing".to_owned(),
            route_key: "asp_testing".to_owned(),
        };
        request.validate().expect("valid registration request");

        let mut invalid = request.clone();
        invalid.child_thread_id = invalid.parent_thread_id.clone();
        assert!(invalid.validate().is_err());
        let mut invalid = request;
        invalid.agent_path = "/root/asp_explorer".to_owned();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn registration_receipt_requires_runtime_grpc_transport_and_generation() {
        let receipt = AgentSessionRegisterReceipt {
            schema_id: AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID.to_owned(),
            schema_version: 1,
            state: "registered".to_owned(),
            platform: "codex".to_owned(),
            project_id: "workspace-1".to_owned(),
            root_session_id: "root-1".to_owned(),
            parent_thread_id: "parent-1".to_owned(),
            child_thread_id: "child-1".to_owned(),
            agent_name: "asp_testing".to_owned(),
            agent_path: "/root/asp_testing".to_owned(),
            route_key: "asp_testing".to_owned(),
            physical_generation: 1,
            registry_owner: "runtime-server-agent-session-registry".to_owned(),
            transport: "grpc-client-frame".to_owned(),
        };
        receipt.validate().expect("current registration receipt");

        let mut invalid = receipt.clone();
        invalid.transport = "legacy-raw-workspace-db".to_owned();
        assert!(invalid.validate().is_err());
        let mut invalid = receipt;
        invalid.physical_generation = 0;
        assert!(invalid.validate().is_err());
    }
}
