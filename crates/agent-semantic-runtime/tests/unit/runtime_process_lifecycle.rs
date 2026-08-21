use crate::runtime_process_lifecycle::{current_process_id, RuntimeProcessLaunchSpec};

#[test]
fn launch_spec_is_runtime_owned_and_pid_is_positive() {
    let spec = RuntimeProcessLaunchSpec { program: "/bin/asp".into(), args: vec!["server".into()], current_dir: None, environment: vec![("ASP_STATE_HOME".into(), "/tmp/state".into())], stderr: "/tmp/stderr".into() };
    assert_eq!(spec.args, vec!["server"]);
    assert!(current_process_id() > 0);
}
