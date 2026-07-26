use std::collections::BTreeSet;

use serde_json::Value;

use crate::search_capability::{
    SearchLoopCapabilityConsumption, SearchLoopCapabilityMutation, SearchLoopCapabilityToken,
    apply_capability_mutations,
};

use crate::search_capability::{
    SearchLoopCapabilityCommand, SearchLoopCapabilityReplayPolicy, SearchLoopCapabilityStatus,
    SearchLoopCapabilityV1, SearchLoopCapabilityValidationError, UncheckedSearchLoopCapabilityV1,
};

const ADVANCE_FIXTURE: &str =
    include_str!("../../../../schemas/fixtures/search-interactive-loop/advance-capability.v1.json");
const POLL_FIXTURE: &str =
    include_str!("../../../../schemas/fixtures/search-interactive-loop/poll-capability.v1.json");
const INVALID_CONSUMED_POLL_FIXTURE: &str = include_str!(
    "../../../../schemas/fixtures/search-interactive-loop/invalid-poll-capability-consumed.v1.json"
);
const CONSUMED_ADVANCE_FIXTURE: &str = include_str!(
    "../../../../schemas/fixtures/search-interactive-loop/consumed-advance-capability.v1.json"
);

fn parse_fixture(fixture: &str) -> UncheckedSearchLoopCapabilityV1 {
    serde_json::from_str(fixture).expect("capability fixture must deserialize")
}

#[test]
fn validates_advance_and_poll_capability_fixtures() {
    let advance = SearchLoopCapabilityV1::from_unchecked(parse_fixture(ADVANCE_FIXTURE))
        .expect("advance capability must validate");
    assert_eq!(advance.command(), SearchLoopCapabilityCommand::Advance);
    assert_eq!(
        advance.replay_policy(),
        SearchLoopCapabilityReplayPolicy::OneShot
    );
    assert_eq!(advance.status(), SearchLoopCapabilityStatus::Active);

    let poll = SearchLoopCapabilityV1::from_unchecked(parse_fixture(POLL_FIXTURE))
        .expect("poll capability must validate");
    assert_eq!(poll.command(), SearchLoopCapabilityCommand::Poll);
    assert_eq!(
        poll.replay_policy(),
        SearchLoopCapabilityReplayPolicy::StateBoundIdempotent
    );
}

#[test]
fn rejects_consumed_poll_capability() {
    let error =
        SearchLoopCapabilityV1::from_unchecked(parse_fixture(INVALID_CONSUMED_POLL_FIXTURE))
            .expect_err("poll capability cannot become consumed");
    assert_eq!(error, SearchLoopCapabilityValidationError::Status);
}

#[test]
fn validates_consumed_advance_recovery_receipt() {
    let consumed = SearchLoopCapabilityV1::from_unchecked(parse_fixture(CONSUMED_ADVANCE_FIXTURE))
        .expect("consumed advance capability must validate");
    assert_eq!(consumed.status(), SearchLoopCapabilityStatus::Consumed);
    assert_eq!(consumed.consumed_at_revision(), Some(4));
    assert_eq!(
        consumed
            .consumption_receipt_ref()
            .expect("consumption receipt")
            .as_str(),
        "receipt:authority-4"
    );
}

#[test]
fn rejects_command_subject_mismatch() {
    let mut value: Value = serde_json::from_str(ADVANCE_FIXTURE).expect("fixture JSON");
    value["commandId"] = Value::String("asp.search.loop.receipt.v1".to_owned());
    value["replayPolicy"] = Value::String("terminal-idempotent".to_owned());
    let unchecked = serde_json::from_value(value).expect("mismatched wire shape must deserialize");
    let error = SearchLoopCapabilityV1::from_unchecked(unchecked)
        .expect_err("command and subject mismatch must fail");
    assert_eq!(error, SearchLoopCapabilityValidationError::CommandSubject);
}

#[test]
fn rejects_non_increasing_expiry() {
    let mut value: Value = serde_json::from_str(ADVANCE_FIXTURE).expect("fixture JSON");
    value["expiresAtUnixMs"] = value["issuedAtUnixMs"].clone();
    let unchecked = serde_json::from_value(value).expect("expiry wire shape must deserialize");
    let error = SearchLoopCapabilityV1::from_unchecked(unchecked)
        .expect_err("non-increasing expiry must fail");
    assert_eq!(error, SearchLoopCapabilityValidationError::Expiry);
}

#[test]
fn persisted_wire_never_contains_raw_token() {
    let token = SearchLoopCapabilityToken::generate().expect("OS CSPRNG must issue a token");
    let raw_token = token.expose_secret().to_owned();
    let mut value: Value = serde_json::from_str(ADVANCE_FIXTURE).expect("fixture JSON");
    value["tokenDigest"] = serde_json::to_value(token.digest()).expect("serialize token digest");
    let capability = SearchLoopCapabilityV1::from_unchecked(
        serde_json::from_value(value).expect("generated capability wire"),
    )
    .expect("advance capability must validate");
    let serialized = serde_json::to_value(capability.wire()).expect("serialize capability");
    assert!(serialized.get("rawToken").is_none());
    assert!(serialized.get("tokenDigest").is_some());
    assert!(!serialized.to_string().contains(&raw_token));
}

#[test]
fn consumes_advance_capability_once() {
    let capability = SearchLoopCapabilityV1::from_unchecked(parse_fixture(ADVANCE_FIXTURE))
        .expect("advance capability must validate");
    let consumption = SearchLoopCapabilityConsumption::new(
        capability.capability_id().clone(),
        capability.token_digest().clone(),
        4,
        agent_semantic_context_product::ProtocolId::parse("receipt:authority-4")
            .expect("receipt id"),
    );
    let mut ledger = vec![capability.wire().clone()];
    apply_capability_mutations(
        &mut ledger,
        &[SearchLoopCapabilityMutation::Consume(consumption.clone())],
    )
    .expect("first consumption must succeed");
    let consumed = SearchLoopCapabilityV1::from_unchecked(ledger[0].clone())
        .expect("consumed capability must remain valid");
    assert_eq!(consumed.status(), SearchLoopCapabilityStatus::Consumed);
    assert_eq!(consumed.consumed_at_revision(), Some(4));
    assert_eq!(
        consumed
            .consumption_receipt_ref()
            .expect("consumption receipt")
            .as_str(),
        "receipt:authority-4"
    );

    let error = apply_capability_mutations(
        &mut ledger,
        &[SearchLoopCapabilityMutation::Consume(consumption)],
    )
    .expect_err("second consumption must fail");
    assert_eq!(error, SearchLoopCapabilityValidationError::Status);
}

#[test]
fn rejects_duplicate_issued_capability() {
    let capability = SearchLoopCapabilityV1::from_unchecked(parse_fixture(ADVANCE_FIXTURE))
        .expect("advance capability must validate");
    let mut ledger = vec![capability.wire().clone()];
    let error = apply_capability_mutations(
        &mut ledger,
        &[SearchLoopCapabilityMutation::Issue(Box::new(capability))],
    )
    .expect_err("duplicate issue must fail");
    assert_eq!(
        error,
        SearchLoopCapabilityValidationError::DuplicateIdentity
    );
}

#[test]
fn opaque_token_hashes_and_redacts() {
    let token = SearchLoopCapabilityToken::generate().expect("OS CSPRNG must issue a token");
    let raw_token = token.expose_secret().to_owned();
    assert_eq!(
        token.digest(),
        agent_semantic_context_product::Digest::from_bytes(token.expose_secret().as_bytes())
    );
    assert_eq!(
        format!("{token:?}"),
        "SearchLoopCapabilityToken([REDACTED])"
    );
    assert!(!format!("{token:?}").contains(&raw_token));
}

#[test]
fn generated_tokens_have_256_bit_opaque_shape_and_are_unique() {
    let mut observed = BTreeSet::new();
    for _ in 0..512 {
        let token = SearchLoopCapabilityToken::generate().expect("OS CSPRNG must issue a token");
        let raw_token = token.expose_secret();
        let payload = raw_token.strip_prefix("capability.").expect("token prefix");
        assert_eq!(payload.len(), 64);
        assert!(
            payload
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        );
        assert_eq!(
            SearchLoopCapabilityToken::parse(raw_token).expect("generated token must parse"),
            token
        );
        assert!(
            observed.insert(raw_token.to_owned()),
            "CSPRNG emitted a duplicate token"
        );
    }
}

#[test]
fn opaque_token_rejects_invalid_shape() {
    for invalid in [
        "capability.short",
        "capability.0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde",
        "capability.0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0",
        "capability.0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeF",
        "capability.advance.abcdefghijklmnopqrstuvwxyz012345",
    ] {
        let error = SearchLoopCapabilityToken::parse(invalid)
            .expect_err("non-canonical capability token must fail");
        assert_eq!(error, SearchLoopCapabilityValidationError::InvalidToken);
    }
}
