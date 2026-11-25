// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

asp_rust::asp_rust_cargo_test_gate!(
    advice = allow,
    config = asp_rust::default_asp_rust_config().with_cargo_test_advice_allow_explanation(
        "scope=agent-semantic-mrr; owner=asp-core; finding_category=info-advisory-docs-and-idioms; why_safe_now=all-warning-and-error-rules-are-strictly-denied; cleanup_trigger=resolve-remaining-info-findings-incrementally"
    )
);
