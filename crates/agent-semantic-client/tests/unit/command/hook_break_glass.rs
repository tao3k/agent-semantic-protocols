// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::break_glass_command;

#[test]
fn mint_uses_clap_to_require_a_typed_defect_and_exact_command() {
    let matches = break_glass_command()
        .try_get_matches_from([
            "asp hook break-glass",
            "mint",
            "--defect-kind",
            "exhausted-non-progress-cycle",
            "--command",
            "asp rust search playbook transport-binding --workspace .",
            "--ttl-seconds",
            "30",
            ".",
        ])
        .expect("valid typed break-glass mint arguments");
    let (_, mint) = matches.subcommand().expect("mint subcommand");
    assert_eq!(
        mint.get_one::<String>("defect-kind").map(String::as_str),
        Some("exhausted-non-progress-cycle")
    );
    assert_eq!(mint.get_one::<u64>("ttl-seconds"), Some(&30));
}

#[test]
fn mint_rejects_untyped_defects_and_unbounded_ttl() {
    for args in [
        vec![
            "asp hook break-glass",
            "mint",
            "--defect-kind",
            "ordinary-policy-deny",
            "--command",
            "cargo test",
        ],
        vec![
            "asp hook break-glass",
            "mint",
            "--defect-kind",
            "exhausted-non-progress-cycle",
            "--command",
            "cargo test",
            "--ttl-seconds",
            "61",
        ],
    ] {
        assert!(break_glass_command().try_get_matches_from(args).is_err());
    }
}
