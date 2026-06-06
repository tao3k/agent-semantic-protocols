use crate::provider_command::support::{
    asp_command, make_executable, provider, temp_project_root, write_activation,
};

#[test]
fn julia_language_facade_rewrites_compiled_provider_guide_commands() {
    let root = temp_project_root("provider-julia-guide-facade");
    let profile_bin_dir = root.join(".profile-bin");
    std::fs::create_dir_all(&profile_bin_dir).expect("create profile bin dir");
    let provider_path = profile_bin_dir.join("asp-julia-harness");
    std::fs::write(
        &provider_path,
        "#!/bin/sh\nprintf '[julia-harness-guide]\\n'\nprintf '|cmd asp-julia-harness guide .\\n'\nprintf '|cmd prime=asp-julia-harness search prime --view seeds .\\n'\nprintf '|pipe <candidate-lines> | asp-julia-harness search ingest owner tests --view seeds .\\n'\nprintf '|cmd doctor=asp-julia-harness agent doctor --json .\\n'\n",
    )
    .expect("write julia guide provider");
    make_executable(&provider_path);

    write_activation(
        &root,
        &[provider("julia", vec![provider_path.display().to_string()])],
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
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
        stdout.contains(
            "[guide] lang=julia provider=julia-lang-project-harness protocol=guide.v1 root=."
        ),
        "{stdout}"
    );
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
    assert!(!stdout.contains("asp-julia-harness"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
