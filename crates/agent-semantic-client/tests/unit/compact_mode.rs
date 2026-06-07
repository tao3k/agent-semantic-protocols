use agent_semantic_client_core::{ClientMethod, ClientRequest};

use crate::compact_mode::{
    CompactOutputMode, request_compact_output_mode, validate_compact_provider_stdout,
};

#[test]
fn infers_frontier_mode_by_default() {
    let request = query_request(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.py:1:10".to_string(),
        ".".to_string(),
    ]);

    assert_eq!(
        request_compact_output_mode(&request),
        CompactOutputMode::Frontier
    );
}

#[test]
fn infers_code_mode_from_code_flag() {
    let request = query_request(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.py:1:10".to_string(),
        "--code".to_string(),
        ".".to_string(),
    ]);

    assert_eq!(
        request_compact_output_mode(&request),
        CompactOutputMode::Code
    );
}

#[test]
fn read_packet_mode_overrides_code_flag() {
    let request = query_request(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.py:1:10".to_string(),
        "--code".to_string(),
        "--view".to_string(),
        "read-packet".to_string(),
        "--json".to_string(),
        ".".to_string(),
    ]);

    assert_eq!(
        request_compact_output_mode(&request),
        CompactOutputMode::ReadPacket
    );
}

#[test]
fn rejects_inline_code_lines_in_frontier_mode() {
    let request = query_request(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.py:1:10".to_string(),
        ".".to_string(),
    ]);
    let stdout = br#"[read-owner] q=src/lib.py
|read path=src/lib.py lineRange=1:2
|code path=src/lib.py lineRange=1:2 text="def bad():\n    pass"
"#;

    let error =
        validate_compact_provider_stdout(&request, stdout).expect_err("inline code rejected");

    assert!(error.contains("provider violated ASP compact frontier mode"));
    assert!(error.contains("`|code` inline source is forbidden"));
}

#[test]
fn rejects_text_fields_in_frontier_mode() {
    let request = query_request(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.py:1:10".to_string(),
        ".".to_string(),
    ]);
    let stdout = br#"[read-owner] q=src/lib.py
|read path=src/lib.py lineRange=1:2 text="def bad():\n    pass"
"#;

    let error =
        validate_compact_provider_stdout(&request, stdout).expect_err("inline text rejected");

    assert!(error.contains("provider violated ASP compact frontier mode"));
    assert!(error.contains("`text` inline source is forbidden"));
}

#[test]
fn allows_inline_source_in_code_and_read_packet_modes() {
    let code_request = query_request(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.py:1:10".to_string(),
        "--code".to_string(),
        ".".to_string(),
    ]);
    let read_packet_request = query_request(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.py:1:10".to_string(),
        "--code".to_string(),
        "--view=read-packet".to_string(),
        "--json".to_string(),
        ".".to_string(),
    ]);
    let stdout = br#"|code path=src/lib.py lineRange=1:2 text="def ok():\n    pass""#;

    validate_compact_provider_stdout(&code_request, stdout).expect("code mode");
    validate_compact_provider_stdout(&read_packet_request, stdout).expect("read-packet mode");
}

fn query_request(forwarded_args: Vec<String>) -> ClientRequest {
    ClientRequest::new(ClientMethod::Query, ".")
        .with_language("python")
        .with_forwarded_args(forwarded_args)
}
