use std::io::Write;
use std::process::Stdio;

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_stdin_provider,
};

#[test]
fn provider_stdin_is_preserved_for_pipe_commands() {
    let root = temp_project_root("provider-stdin-facade");
    let bin_dir = root.join(".bin");
    write_stdin_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let mut child = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args(["rust", "search", "ingest", "."])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run asp rust search ingest");
    child
        .stdin
        .as_mut()
        .expect("facade stdin")
        .write_all(b"src/lib.rs:10:HookDecision\n")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for facade");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "stdin=src/lib.rs:10:HookDecision\n"
    );
    let _ = std::fs::remove_dir_all(root);
}
