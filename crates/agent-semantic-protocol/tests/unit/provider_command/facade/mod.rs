mod language {
    include!("language.rs");

    #[test]
    fn language_facade_forwards_agent_doctor_to_provider() {
        let root =
            crate::provider_command::support::temp_project_root("language-agent-doctor-facade");
        let bin_dir = root.join(".bin");
        std::fs::create_dir_all(&bin_dir).expect("create bin dir");

        let provider_path = bin_dir.join("rs-harness");
        std::fs::write(&provider_path, "#!/bin/sh\nprintf 'doctor:%s\n' \"$*\"\n")
            .expect("write provider");
        let mut permissions = std::fs::metadata(&provider_path)
            .expect("provider metadata")
            .permissions();
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o755);
        }
        std::fs::set_permissions(&provider_path, permissions).expect("provider permissions");

        crate::provider_command::support::write_activation(
            &root,
            &[crate::provider_command::support::provider(
                "rust",
                Vec::new(),
            )],
        );
        let output = crate::provider_command::support::asp_command(&root)
            .env(
                "PATH",
                crate::provider_command::support::prepend_path(&bin_dir),
            )
            .args(["rust", "agent", "doctor", "--json", "."])
            .output()
            .expect("run asp");
        assert!(
            output.status.success(),
            "status={:?} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(
            stdout.contains("doctor:agent doctor --json ."),
            "stdout={stdout}"
        );
        std::fs::remove_dir_all(root).expect("remove temp root");
    }
}
mod pipe;
mod provider_invocation;
mod rewrite;
mod roots;
