mod language {
    include!("language.rs");

    #[test]
    fn language_facade_forwards_agent_doctor_to_provider() {
        let root =
            crate::provider_command::support::temp_project_root("language-agent-doctor-facade");
        let bin_dir = crate::provider_command::support::home_local_bin(&root);
        let cache_home = root.join(".cache");
        std::fs::create_dir_all(&bin_dir).expect("create bin dir");

        let provider_path = bin_dir.join("rs-harness");
        std::fs::write(&provider_path, "#!/bin/sh\nprintf 'doctor:%s\n' \"$*\"\n")
            .expect("write provider");
        crate::provider_command::support::make_executable(&provider_path);

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
            .env("PRJ_CACHE_HOME", &cache_home)
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
            stdout.contains("doctor:agent doctor --json"),
            "stdout={stdout}"
        );
        std::fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn language_facade_prefers_home_local_provider_over_project_bin() {
        let root =
            crate::provider_command::support::temp_project_root("language-agent-doctor-bin-order");
        let bin_dir = root.join(".bin");
        let home_bin_dir = crate::provider_command::support::home_local_bin(&root);
        let cache_home = root.join(".cache");
        std::fs::create_dir_all(&bin_dir).expect("create bin dir");
        std::fs::create_dir_all(&home_bin_dir).expect("create home bin dir");

        let project_provider = bin_dir.join("rs-harness");
        std::fs::write(
            &project_provider,
            "#!/bin/sh\nprintf 'project-bin:%s\n' \"$*\"\n",
        )
        .expect("write project provider");
        crate::provider_command::support::make_executable(&project_provider);
        let home_provider = home_bin_dir.join("rs-harness");
        std::fs::write(
            &home_provider,
            "#!/bin/sh\nprintf 'home-local:%s\n' \"$*\"\n",
        )
        .expect("write home provider");
        crate::provider_command::support::make_executable(&home_provider);

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
            .env("PRJ_CACHE_HOME", &cache_home)
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
            stdout.contains("home-local:agent doctor --json"),
            "stdout={stdout}"
        );
        assert!(!stdout.contains("project-bin:"), "stdout={stdout}");
        std::fs::remove_dir_all(root).expect("remove temp root");
    }
}
mod client_commands;
mod dependency_seed;
mod document;
mod guide;
mod performance;
mod pipe;
mod provider_invocation;
mod rewrite;
mod root_language;
mod roots;
mod search_output_snapshots;
