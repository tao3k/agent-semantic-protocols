use crate::byte_text;

#[test]
fn byte_text_helpers_preserve_byte_oriented_boundaries() {
    assert_eq!(
        byte_text::split_lf_or_nul_records(b" src/lib.rs \n\ttests/a.rs\0").collect::<Vec<_>>(),
        vec![
            b"src/lib.rs".as_slice(),
            b"tests/a.rs".as_slice(),
            b"".as_slice()
        ]
    );
    assert_eq!(
        byte_text::split_lf_lines(b"first\r\nsecond\n").collect::<Vec<_>>(),
        vec![b"first".as_slice(), b"second".as_slice(), b"".as_slice()]
    );
    assert_eq!(
        byte_text::line_slices(b"first\r\nsecond"),
        vec![b"first".as_slice(), b"second".as_slice()]
    );
    assert_eq!(byte_text::find_byte(b':', b"path:12:text"), Some(4));
    assert!(byte_text::lossy_string(b"\xffterm").contains("term"));
}
