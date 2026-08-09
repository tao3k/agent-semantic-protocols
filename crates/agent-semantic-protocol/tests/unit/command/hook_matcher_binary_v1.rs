use super::{
    MatcherSection, MatcherSectionKind, encode_matcher_bundle, matcher_section_key,
    select_matcher_section,
};

fn fixture_sections() -> Vec<MatcherSection> {
    vec![
        MatcherSection {
            kind: MatcherSectionKind::Complete,
            key: [0; 8],
            bytes: b"complete".to_vec(),
        },
        MatcherSection {
            kind: MatcherSectionKind::DirectRead,
            key: matcher_section_key(".rs"),
            bytes: b"direct-rust".to_vec(),
        },
        MatcherSection {
            kind: MatcherSectionKind::ShellRead,
            key: matcher_section_key(".rs"),
            bytes: b"shell-rust".to_vec(),
        },
        MatcherSection {
            kind: MatcherSectionKind::ShellCommand,
            key: [0; 8],
            bytes: b"shell-command-profiles".to_vec(),
        },
    ]
}

#[test]
fn binary_v1_index_selects_one_typed_section_without_decoding_siblings() {
    let bundle = encode_matcher_bundle(&fixture_sections()).expect("encode matcher bundle");

    assert_eq!(
        select_matcher_section(&bundle, MatcherSectionKind::ShellCommand, [0; 8])
            .expect("select command-profile section"),
        Some(b"shell-command-profiles".as_slice())
    );
    assert_eq!(
        select_matcher_section(
            &bundle,
            MatcherSectionKind::DirectRead,
            matcher_section_key(".rs"),
        )
        .expect("select direct section"),
        Some(b"direct-rust".as_slice())
    );
    assert_eq!(
        select_matcher_section(
            &bundle,
            MatcherSectionKind::ShellRead,
            matcher_section_key(".rs"),
        )
        .expect("select shell section"),
        Some(b"shell-rust".as_slice())
    );
    assert_eq!(
        select_matcher_section(
            &bundle,
            MatcherSectionKind::DirectRead,
            matcher_section_key(".unknown"),
        )
        .expect("select absent section"),
        None
    );
}

#[test]
fn binary_v1_selected_section_corruption_fails_closed() {
    let mut bundle = encode_matcher_bundle(&fixture_sections()).expect("encode matcher bundle");
    let last = bundle.last_mut().expect("bundle payload");
    *last ^= 0xff;

    let error = select_matcher_section(&bundle, MatcherSectionKind::ShellCommand, [0; 8])
        .expect_err("selected corrupt section must fail closed");

    assert!(error.contains("digest mismatch"), "{error}");
}
