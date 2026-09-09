// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[test]
fn large_library_sandtables_have_hard_elapsed_gates() {
    super::sandtable_gates::large_library_sandtables_have_hard_elapsed_gates();
}

#[test]
fn julia_dataframes_sandtable_batch_execution_stays_inside_hard_gates() {
    super::sandtable_gates::julia_dataframes_sandtable_batch_execution_stays_inside_hard_gates();
}

#[test]
fn python_sandtable_runner_does_not_resolve_provider_binaries() {
    super::sandtable_gates::python_sandtable_runner_does_not_resolve_provider_binaries();
}
