use serde::{Deserialize, Serialize};

pub const CODEX_THREAD_REFERENCE_SCHEMA_ID: &str =
    "agent.semantic-protocols.codex-thread-reference";
pub const CODEX_THREAD_REFERENCE_SCHEMA_VERSION: &str = "1";
pub const CODEX_THREAD_TOOL_CALL_SCHEMA_ID: &str =
    "agent.semantic-protocols.codex-thread-management-tool-call";
pub const CODEX_THREAD_TOOL_CALL_SCHEMA_VERSION: &str = "1";
pub const CODEX_THREAD_NAMESPACE: &str = "codex_app";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexThreadReference {
    pub schema_id: String,
    pub schema_version: String,
    pub thread_id: String,
    pub deeplink: String,
}

impl CodexThreadReference {
    pub fn new(thread_id: impl Into<String>) -> Result<Self, String> {
        let thread_id = thread_id.into();
        validate_thread_id(&thread_id)?;
        Ok(Self {
            schema_id: CODEX_THREAD_REFERENCE_SCHEMA_ID.to_owned(),
            schema_version: CODEX_THREAD_REFERENCE_SCHEMA_VERSION.to_owned(),
            deeplink: format!("codex://threads/{thread_id}"),
            thread_id,
        })
    }

    pub fn parse_deeplink(deeplink: &str) -> Result<Self, String> {
        let thread_id = deeplink
            .strip_prefix("codex://threads/")
            .ok_or_else(|| "Codex thread deeplink must start with codex://threads/".to_owned())?;
        if thread_id == "new" || thread_id.contains(['/', '?', '#']) {
            return Err(
                "Codex existing-thread deeplink must contain exactly one thread UUID".to_owned(),
            );
        }
        Self::new(thread_id)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != CODEX_THREAD_REFERENCE_SCHEMA_ID
            || self.schema_version != CODEX_THREAD_REFERENCE_SCHEMA_VERSION
        {
            return Err("unsupported Codex thread reference schema".to_owned());
        }
        validate_thread_id(&self.thread_id)?;
        let expected = format!("codex://threads/{}", self.thread_id);
        if self.deeplink != expected {
            return Err("Codex thread deeplink and threadId identify different tasks".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexThreadToolCall {
    pub schema_id: String,
    pub schema_version: String,
    pub namespace: String,
    #[serde(flatten)]
    pub operation: CodexThreadOperation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "toolName", content = "toolInput", rename_all = "snake_case")]
pub enum CodexThreadOperation {
    NavigateToCodexPage(ThreadTargetInput),
    ReadThread(ReadThreadInput),
    SendMessageToThread(SendMessageToThreadInput),
    WaitThreads(WaitThreadsInput),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThreadTargetInput {
    pub thread_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadThreadInput {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_outputs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_chars_per_item: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_limit: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendMessageToThreadInput {
    pub thread_id: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaitThreadsInput {
    pub targets: Vec<WaitThreadTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaitThreadTarget {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_cursor: Option<String>,
}

impl CodexThreadToolCall {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != CODEX_THREAD_TOOL_CALL_SCHEMA_ID
            || self.schema_version != CODEX_THREAD_TOOL_CALL_SCHEMA_VERSION
            || self.namespace != CODEX_THREAD_NAMESPACE
        {
            return Err("unsupported Codex thread-management tool call schema".to_owned());
        }
        match &self.operation {
            CodexThreadOperation::NavigateToCodexPage(input) => {
                validate_thread_id(&input.thread_id)
            }
            CodexThreadOperation::ReadThread(input) => validate_thread_id(&input.thread_id),
            CodexThreadOperation::SendMessageToThread(input) => {
                validate_thread_id(&input.thread_id)?;
                require_non_blank("prompt", &input.prompt)
            }
            CodexThreadOperation::WaitThreads(input) => {
                if input.targets.is_empty() || input.targets.len() > 8 {
                    return Err("wait_threads requires between one and eight targets".to_owned());
                }
                for target in &input.targets {
                    validate_thread_id(&target.thread_id)?;
                }
                Ok(())
            }
        }
    }
}

fn validate_thread_id(thread_id: &str) -> Result<(), String> {
    let bytes = thread_id.as_bytes();
    let valid = bytes.len() == 36
        && [8, 13, 18, 23]
            .into_iter()
            .all(|index| bytes[index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit());
    if !valid {
        return Err("Codex threadId must be a canonical UUID".to_owned());
    }
    Ok(())
}

fn require_non_blank(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("Codex thread-management {field} must not be blank"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/codex_threads.rs"]
mod tests;
