use std::path::{Path, PathBuf};
use std::process::Command;

use semantic_agent_hook::parse_profiles;

use crate::rust_harness_profile::support::temp_project_root;

#[test]
fn cli_install_writes_workspace_julia_profile_route() {
    let root = temp_project_root("install-julia");
    write_workspace_julia_harness(&root);
    let provider_path = write_fake_workspace_julia_binary(&root);
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .env("PATH", &provider_path)
        .env("CODEX_HOME", root.join(".codex-home"))
        .args([
            "install",
            "--client",
            "codex",
            root.to_str().expect("utf8 temp root"),
        ])
        .output()
        .expect("run semantic-agent-hook install");

    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let profiles = std::fs::read_to_string(root.join(".codex/semantic-agent-hook/profiles.json"))
        .expect("installed profile registry");
    let registry = parse_profiles(&profiles).expect("valid installed profile registry");
    let julia = registry
        .profiles
        .iter()
        .find(|profile| profile.language_id == "julia")
        .expect("julia profile");
    assert_eq!(julia.binary, "julia-project-harness");
    assert_eq!(
        julia.provider_command_prefix,
        [
            "julia",
            "--project=languages/JuliaLangProjectHarness.jl",
            "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl"
        ]
    );
    assert_eq!(
        julia.commands.owner.argv,
        [
            "julia",
            "--project=languages/JuliaLangProjectHarness.jl",
            "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl",
            "search",
            "owner",
            "{path}",
            "--view",
            "seeds",
            "."
        ]
    );

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn write_workspace_julia_harness(root: &Path) {
    let harness_root = root.join("languages/JuliaLangProjectHarness.jl");
    std::fs::create_dir_all(harness_root.join("bin")).expect("create workspace Julia harness");
    std::fs::write(
        harness_root.join("Project.toml"),
        "name = \"JuliaLangProjectHarness\"\n",
    )
    .expect("write workspace Julia Project.toml");
    std::fs::write(
        harness_root.join("bin/julia-project-harness.jl"),
        "#!/usr/bin/env julia\n",
    )
    .expect("write workspace Julia harness bin");
}

fn write_fake_workspace_julia_binary(root: &Path) -> PathBuf {
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create fake Julia bin dir");
    let path = bin_dir.join("julia");
    std::fs::write(
        &path,
        "#!/bin/sh\nif [ \"$1\" = \"--project=languages/JuliaLangProjectHarness.jl\" ] && [ \"$2\" = \"languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl\" ] && [ \"$3\" = \"agent\" ] && [ \"$4\" = \"guide\" ]; then\n  printf '%s\\n' '[julia-harness-guide]'\n  exit 0\nfi\nexit 0\n",
    )
    .expect("write fake Julia binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path)
            .expect("fake Julia metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod fake Julia");
    }
    bin_dir
}
