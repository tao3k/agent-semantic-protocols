use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, provider, temp_project_root, write_activation,
    write_echo_provider, write_runtime_profiles,
};

#[test]
fn language_facade_guide_routes_to_provider_runtime_profile() {
    let root = temp_project_root("provider-guide-facade");
    let profile_bin_dir = root.join(".profile-bin");
    let path_bin_dir = root.join(".path-bin");
    write_echo_provider(&profile_bin_dir, "rs-harness", "profile");
    std::fs::create_dir_all(&path_bin_dir).expect("create path bin dir");
    let path_provider = path_bin_dir.join("rs-harness");
    std::fs::write(
        &path_provider,
        "#!/bin/sh\nprintf 'path provider should not run\\n' >&2\nexit 42\n",
    )
    .expect("write path provider");
    make_executable(&path_provider);

    write_activation(&root, &[provider("rust", Vec::new())]);
    write_runtime_profiles(
        &root,
        "rust",
        vec![profile_bin_dir.join("rs-harness").display().to_string()],
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&path_bin_dir))
        .args(["rust", "guide", "."])
        .output()
        .expect("run asp rust guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("profile args=[guide][.]\n"), "{stdout}");
    assert!(
        stdout.contains("|cmd agent-doctor=asp rust agent doctor --json ."),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn julia_language_facade_rewrites_compiled_provider_guide_commands() {
    let root = temp_project_root("provider-julia-guide-facade");
    let profile_bin_dir = root.join(".profile-bin");
    std::fs::create_dir_all(&profile_bin_dir).expect("create profile bin dir");
    let provider_path = profile_bin_dir.join("aslp-julia-harness");
    std::fs::write(
        &provider_path,
        "#!/bin/sh\nprintf '[julia-harness-guide]\\n'\nprintf '|cmd aslp-julia-harness guide .\\n'\nprintf '|cmd prime=aslp-julia-harness search prime --view seeds .\\n'\nprintf '|pipe <candidate-lines> | aslp-julia-harness search ingest owner tests --view seeds .\\n'\nprintf '|cmd doctor=aslp-julia-harness agent doctor --json .\\n'\n",
    )
    .expect("write julia guide provider");
    make_executable(&provider_path);

    write_activation(&root, &[provider("julia", Vec::new())]);
    write_runtime_profiles(&root, "julia", vec![provider_path.display().to_string()]);

    let output = asp_command(&root)
        .args(["julia", "guide", "."])
        .output()
        .expect("run asp julia guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("|cmd prime=asp julia search prime --view seeds ."),
        "{stdout}"
    );
    assert!(stdout.contains("|cmd asp julia guide ."), "{stdout}");
    assert!(
        stdout.contains(
            "|pipe <candidate-lines> | asp julia search ingest owner tests --view seeds ."
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("|cmd doctor=asp julia agent doctor --json ."),
        "{stdout}"
    );
    assert!(!stdout.contains("aslp-julia-harness"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_rejects_agent_guide_alias() {
    let root = temp_project_root("provider-agent-guide-alias");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .args(["rust", "agent", "guide", "."])
        .output()
        .expect("run asp rust agent guide");

    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains(
            "usage: asp <rust|typescript|python|julia> <guide|search|query|check|agent doctor|ast-patch|evidence> ..."
        ),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}
