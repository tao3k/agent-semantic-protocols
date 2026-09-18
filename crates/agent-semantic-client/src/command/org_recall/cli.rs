// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub(crate) fn run_org_recall_command(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help" | "help"))
    {
        println!("{}", recall_usage());
        return Ok(());
    }
    if !matches!(args.first().map(String::as_str), Some("plans")) {
        return Err("asp org recall is unused; the legacy Artifacts-backed recall contract has been retired pending a Runtime-owned V1 redesign".to_owned());
    }
    Err(
        "state=unused reasonKind=org-plan-recall-unused `asp org recall plans` still targets the retired orgArtifacts/flow/plans model; redesign it against the current Runtime Artifacts authority before use"
            .to_owned(),
    )
}

fn recall_usage() -> &'static str {
    "usage: asp org recall plans\n\nstate: UNUSED\nreasonKind: org-plan-recall-unused\n\nThe legacy command targets the retired orgArtifacts/flow/plans model. It is intentionally unavailable until recall is redesigned against the current Runtime Artifacts authority."
}
