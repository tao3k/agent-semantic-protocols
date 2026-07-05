use crate::provider_command::support::{
    asp_command, make_executable, provider, temp_project_root, write_activation,
};

#[test]
fn language_facade_guide_normalizes_provider_specific_header() {
    let root = temp_project_root("provider-specific-guide-header-facade");
    let profile_bin_dir = root.join(".profile-bin");
    std::fs::create_dir_all(&profile_bin_dir).expect("create profile bin dir");
    let provider_path = profile_bin_dir.join("ts-harness");
    std::fs::write(
        &provider_path,
        "#!/bin/sh\nprintf '[ts-harness-guide] project=/tmp/project\\n'\nprintf '|cmd lexical=ts-harness search lexical <query> owner tests --workspace . --view seeds\\n'\n",
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_provider_bin_override(&root, "typescript", &provider_path);

    write_activation(
        &root,
        &[provider(
            "typescript",
            vec![provider_path.display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["typescript", "guide", "."])
        .output()
        .expect("run asp typescript guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("[guide] lang=typescript provider=ts-harness protocol=guide.v1 root=."),
        "{stdout}"
    );
    assert!(!stdout.contains("[ts-harness-guide]"), "{stdout}");
    assert!(
        stdout.contains(
            "|cmd lexical=asp typescript search lexical <query> owner tests --workspace . --view seeds"
        ),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_guide_normalizes_provider_agent_guide_header() {
    let root = temp_project_root("provider-guide-header-facade");
    let profile_bin_dir = root.join(".profile-bin");
    std::fs::create_dir_all(&profile_bin_dir).expect("create profile bin dir");
    let provider_path = profile_bin_dir.join("rs-harness");
    std::fs::write(
        &provider_path,
        "#!/bin/sh\nprintf '[agent-guide] lang=rust provider=asp-rust protocol=agent-guide.v1 root=.\\n'\nprintf '|refer query-guide=\"query guide .\"\\n'\n",
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_provider_bin_override(&root, "rust", &provider_path);

    write_activation(
        &root,
        &[provider("rust", vec![provider_path.display().to_string()])],
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rust", "guide", "."])
        .output()
        .expect("run asp rust guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("[guide] lang=rust provider=asp-rust protocol=guide.v1 root=."),
        "{stdout}"
    );
    assert!(!stdout.contains("[agent-guide]"), "{stdout}");
    assert!(!stdout.contains("protocol=agent-guide.v1"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

fn write_provider_bin_override(
    root: &std::path::Path,
    language_id: &str,
    provider_bin: &std::path::Path,
) {
    std::fs::write(
        root.join("asp.toml"),
        format!(
            "[languages.{language_id}]\nbin = \"{}\"\n",
            provider_bin.display()
        ),
    )
    .expect("write asp.toml provider override");
}
