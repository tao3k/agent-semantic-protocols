#[path = "../../src/source_dump_range.rs"]
mod source_dump_range_impl;

use source_dump_range_impl::line_range_source_paths;

fn expected_paths(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|path| (*path).to_string()).collect()
}

#[test]
fn extracts_deterministic_source_ranges() {
    for (command, expected) in [
        ("sed -n '1,40p' src/lib.rs", vec!["src/lib.rs:1:40"]),
        (
            "awk 'NR>=115 && NR<=240' src/lib.rs",
            vec!["src/lib.rs:115:240"],
        ),
        ("awk 'NR==42' src/lib.rs", vec!["src/lib.rs:42:42"]),
        ("head -n 40 src/lib.rs", vec!["src/lib.rs:1:40"]),
        (
            "head -n 240 src/lib.rs | tail -n 126",
            vec!["src/lib.rs:115:240"],
        ),
        (
            "tail -n +115 src/lib.rs | head -n 126",
            vec!["src/lib.rs:115:240"],
        ),
        (
            "nl -ba src/lib.rs | sed -n '115,240p'",
            vec!["src/lib.rs:115:240"],
        ),
        ("head -n 20 src/cli/main.ts", vec!["src/cli/main.ts:1:20"]),
        (
            "tail -n +10 packages/python/src/tools/semantic_sandtable/receipts.py | head -n 21",
            vec!["packages/python/src/tools/semantic_sandtable/receipts.py:10:30"],
        ),
    ] {
        assert_eq!(
            line_range_source_paths(command),
            expected_paths(&expected),
            "{command}"
        );
    }
}

#[test]
fn refuses_underdetermined_or_broad_ranges() {
    for command in [
        "tail -n 40 src/lib.rs",
        "tail -40 src/lib.rs | head -n 10",
        "sed -n '1,20p' **/*.rs",
        "head -n 20 'src/**/*.tsx'",
    ] {
        assert!(line_range_source_paths(command).is_empty(), "{command}");
    }
}
