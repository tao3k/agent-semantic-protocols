// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::CODEX_THREAD_NAMESPACE;
use super::CODEX_THREAD_TOOL_CALL_SCHEMA_ID;
use super::CODEX_THREAD_TOOL_CALL_SCHEMA_VERSION;
use super::CodexThreadOperation;
use super::CodexThreadReference;
use super::CodexThreadToolCall;
use super::SendMessageToThreadInput;

const THREAD_ID: &str = "01a055c3-6a84-7332-b76c-70b07801f029";

#[test]
fn existing_thread_deeplink_roundtrips_one_codex_thread_id() {
    let reference = CodexThreadReference::new(THREAD_ID).expect("thread reference");
    assert_eq!(reference.deeplink, format!("codex://threads/{THREAD_ID}"));
    assert_eq!(
        CodexThreadReference::parse_deeplink(&reference.deeplink),
        Ok(reference)
    );
}

#[test]
fn new_thread_and_mismatched_deeplinks_are_not_existing_thread_references() {
    assert!(CodexThreadReference::parse_deeplink("codex://threads/new?path=/tmp").is_err());
    let mut reference = CodexThreadReference::new(THREAD_ID).expect("thread reference");
    reference.deeplink = "codex://threads/019fc51d-36ec-7890-b809-fcbd8f48fc28".to_owned();
    assert!(reference.validate().is_err());
}

#[test]
fn thread_tool_calls_use_thread_id_not_agent_path_or_session_id() {
    let call = CodexThreadToolCall {
        schema_id: CODEX_THREAD_TOOL_CALL_SCHEMA_ID.to_owned(),
        schema_version: CODEX_THREAD_TOOL_CALL_SCHEMA_VERSION.to_owned(),
        namespace: CODEX_THREAD_NAMESPACE.to_owned(),
        operation: CodexThreadOperation::SendMessageToThread(SendMessageToThreadInput {
            thread_id: THREAD_ID.into(),
            prompt: "Continue the existing task.".to_owned(),
            host_id: None,
            model: None,
            thinking: None,
        }),
    };
    assert_eq!(call.validate(), Ok(()));
}
