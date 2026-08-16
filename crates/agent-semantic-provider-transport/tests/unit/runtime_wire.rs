use super::*;

#[test]
fn request_frame_round_trips_with_typed_identity() {
    let frame = ProviderRuntimeRequestFrame::new(
        "request-1",
        "provider-search",
        br#"{"args":["owner","items"]}"#,
    )
    .expect("request frame");
    let encoded = serde_json::to_vec(&frame).expect("encode request frame");
    let decoded: ProviderRuntimeRequestFrame =
        serde_json::from_slice(&encoded).expect("decode request frame");
    decoded.validate().expect("validate request frame");
    assert_eq!(decoded, frame);
}

#[test]
fn response_frame_enforces_outcome_payload_exclusivity() {
    ProviderRuntimeResponseFrame::ready("request-1", br#"{"entries":[]}"#)
        .validate()
        .expect("Ready response");
    ProviderRuntimeResponseFrame::error("request-2", "typed provider error")
        .validate()
        .expect("Error response");

    let invalid = ProviderRuntimeResponseFrame {
        schema_id: PROVIDER_RUNTIME_RESPONSE_FRAME_SCHEMA_ID.to_owned(),
        schema_version: PROVIDER_RUNTIME_FRAME_SCHEMA_VERSION.to_owned(),
        request_id: "request-3".to_owned(),
        outcome: ProviderRuntimeResponseOutcome::Ready,
        payload: None,
        error: Some("ambiguous".to_owned()),
    };
    assert_eq!(
        invalid
            .validate()
            .expect_err("ambiguous response must fail"),
        "provider runtime response frame outcome/payload mismatch"
    );
}
