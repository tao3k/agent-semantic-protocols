// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::runtime_process_lifecycle::RuntimeProcessLaunchSpec;
use crate::runtime_process_lifecycle::current_process_id;
use crate::runtime_process_lifecycle::process_id_is_alive;

#[test]
fn launch_spec_is_runtime_owned_and_pid_is_positive() {
    let spec = RuntimeProcessLaunchSpec {
        program: "/bin/asp".into(),
        args: vec!["server".into()],
        current_dir: None,
        environment: vec![("ASP_STATE_HOME".into(), "/tmp/state".into())],
        stderr: "/tmp/stderr".into(),
    };
    assert_eq!(spec.args, vec!["server"]);
    assert!(current_process_id() > 0);
}

#[tokio::test]
async fn current_process_is_a_live_runtime_owner() {
    assert!(process_id_is_alive(current_process_id()).await);
}

#[tokio::test]
async fn unrepresentable_process_id_is_not_a_live_runtime_owner() {
    assert!(!process_id_is_alive(u32::MAX).await);
}
