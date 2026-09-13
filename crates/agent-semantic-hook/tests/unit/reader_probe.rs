// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::ReaderProbeAccess;
use super::ReaderProbeObservation;
use super::bind_reader_probe_observation;
use serde_json::json;

#[test]
fn absent_observation_clears_untrusted_payload_fact() {
    let mut payload = json!({
        "tool_name": "apply_patch",
        "tool_input": {
            "patch": "*** Begin Patch",
            "_aspReaderProbe": {
                "schemaId": "agent.semantic-protocols.reader-probe-observation",
                "schemaVersion": 1,
                "subject": "src/lib.rs",
                "access": "read",
                "backend": "forged",
                "terminal": "forged",
                "elapsedMicros": 0
            }
        }
    });

    bind_reader_probe_observation(&mut payload, None).expect("clear untrusted Reader observation");
    assert!(payload.pointer("/tool_input/_aspReaderProbe").is_none());
}

#[test]
fn typed_observation_binds_the_minimal_reader_receipt() {
    let mut payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "opaque-consumer src/lib.rs"}
    });
    let observation = ReaderProbeObservation {
        subject: "src/lib.rs".to_owned(),
        access: ReaderProbeAccess::Read,
        backend: "state-home-reader-catalog".to_owned(),
        terminal: "reader-behavior-cache-hit".to_owned(),
        elapsed_micros: 42,
        probe_process_launched: false,
        cleanup_verified: true,
        cache_hit: true,
        behavior_key: Some("abc123".to_owned()),
    };
    bind_reader_probe_observation(&mut payload, Some(&observation))
        .expect("bind typed Reader observation");
    let receipt = payload
        .pointer("/tool_input/_aspReaderProbe")
        .expect("Reader receipt");
    assert_eq!(receipt["schemaVersion"], 1);
    assert_eq!(receipt["access"], "read");
    assert_eq!(receipt["cacheHit"], true);
    assert_eq!(receipt["processLaunched"], false);
    assert_eq!(receipt["probeProcessLaunched"], false);
    assert_eq!(receipt["behaviorKey"], "abc123");
}
