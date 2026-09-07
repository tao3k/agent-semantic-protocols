// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::is_dispatch_resident_command_choice;

#[test]
fn only_current_ready_dispatch_choice_projects_claim() {
    assert!(is_dispatch_resident_command_choice(
        "dispatch-resident-command"
    ));
    assert!(!is_dispatch_resident_command_choice(
        "send-denied-asp-command"
    ));
    assert!(!is_dispatch_resident_command_choice(
        "audit-host-agent-tree-before-live-target-rebind"
    ));
}
