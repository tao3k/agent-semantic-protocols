use super::decode_http_response;

#[test]
fn non_success_http_response_preserves_status_and_reason_kind() {
    let error = decode_http_response(
        409,
        br#"{"reasonKind":"client-workspace-catalog-unavailable","message":"workspace is not admitted"}"#,
    )
    .expect_err("non-success HTTP response");

    assert_eq!(
        error,
        "ASP Client HTTP error status=409 reasonKind=client-workspace-catalog-unavailable message=workspace is not admitted"
    );
}

#[test]
fn malformed_success_body_is_a_frame_decode_failure() {
    let error = decode_http_response(200, br#"{"state":"not-a-frame"}"#)
        .expect_err("success body must be a typed frame");
    assert!(error.starts_with("decode client frame:"));
}
