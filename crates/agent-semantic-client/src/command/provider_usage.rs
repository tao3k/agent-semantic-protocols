// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::provider_selector::registered_language_facades_line;

pub(super) fn is_guide(args: &[String]) -> bool {
    args.first().is_some_and(|command| command == "guide")
}

pub(super) fn provider_usage() -> String {
    format!(
        "usage: asp <{}> [--help|--version] <guide|cache|info|bench|projection|agent doctor|ast-patch> ...\nprojection: import --owner <relative-owner-path> --workspace <root>\nSearch and Query are Playbook-only root operations: `asp search playbook '<scheme-expression>'` and `asp query playbook --language <producer|...> --selector <parser-owned-selector>...`. Language policy verification is build/CI-owned; typed Runtime receipts carry result provenance.",
        registered_language_facades_line()
    )
}

pub(super) fn guide_usage(language_id: &str) -> String {
    format!(
        "usage: asp {language_id} guide [--help] [--workspace <root>]\n\nPrints the low-frequency provider-owned agent tool map. Search Playbook selects this provider inside `(producers (language {language_id}))` or `(producers (documents {language_id}))`."
    )
}

#[cfg(test)]
#[path = "../../tests/unit/command/provider_usage.rs"]
mod tests;
