// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{HOOK_STDIN_MAX_BYTES, append_chunk, json_frame_is_complete, read_one_json_frame};

struct OneFrameWithoutEof {
    frame: Option<&'static [u8]>,
}

impl std::io::Read for OneFrameWithoutEof {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let frame = self
            .frame
            .take()
            .expect("reader must not be polled after one complete JSON frame");
        output[..frame.len()].copy_from_slice(frame);
        Ok(frame.len())
    }
}

#[test]
fn complete_json_frame_does_not_require_eof() {
    assert!(json_frame_is_complete(br#"{"tool_name":"apply_patch"}"#).unwrap());
    assert!(!json_frame_is_complete(br#"{"tool_name":"apply"#).unwrap());
}

#[test]
fn framed_reader_returns_before_polling_for_eof() {
    let frame = br#"{"tool_name":"apply_patch"}"#;
    let rendered = read_one_json_frame(OneFrameWithoutEof { frame: Some(frame) }).unwrap();
    assert_eq!(rendered.as_bytes(), frame);
}

#[test]
fn rejects_multiple_json_frames() {
    let error = json_frame_is_complete(br#"{}{}"#).expect_err("one frame only");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn payload_bound_is_fail_closed() {
    let mut bytes = vec![b'x'; HOOK_STDIN_MAX_BYTES];
    let error = append_chunk(&mut bytes, b"x").expect_err("payload exceeds bound");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}
