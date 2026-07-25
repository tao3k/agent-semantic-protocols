use std::path::Path;

use crate::provider_command::support::{
    asp_command, install_state_home_provider, prepend_path, provider, state_home,
    state_runtime_bin, temp_project_root, write_activation, write_echo_provider,
    write_marker_provider, write_pwd_provider,
};

#[test]
fn rust_search_facade_fans_out_multiple_trailing_scope_roots() {
    let root = temp_project_root("rust-search-facade-multi-scope");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("crates/agent-semantic-hook")).expect("create hook scope");
    std::fs::create_dir_all(root.join("crates/agent-semantic-protocol"))
        .expect("create protocol scope");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    install_state_home_provider(&root, "rust", &bin_dir.join("rs-harness"));
    write_activation(&root, &[provider("rust", Vec::new())]);
    let activation_path =
        agent_semantic_runtime::project_state_paths_with_state_home(&root, state_home(&root))
            .expect("State Home paths")
            .activation_path;
    let activation = std::fs::read_to_string(&activation_path).expect("read activation");
    let activation =
        agent_semantic_hook::parse_hook_activation(&activation).expect("parse activation");
    let rust_provider = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("rust provider activation");
    assert!(
        !rust_provider
            .query_pack_descriptor
            .descriptor_id()
            .is_empty(),
        "rust provider activation must carry query-pack descriptor"
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "--query-set",
            "reasonKind",
            "--query-set",
            "RawBroadSearch",
            "owner",
            "tests",
            "--view",
            "seeds",
            "crates/agent-semantic-hook",
            "crates/agent-semantic-protocol",
        ])
        .output()
        .expect("run asp rust search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        concat!(
            "rs args=[search][lexical][--query-set][reasonKind][--query-set][RawBroadSearch][owner][tests][--view][seeds][crates/agent-semantic-hook]\n",
            "rs args=[search][lexical][--query-set][reasonKind][--query-set][RawBroadSearch][owner][tests][--view][seeds][crates/agent-semantic-protocol]\n",
        )
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rust_search_facade_rejects_multiple_workspace_flags() {
    let root = temp_project_root("rust-search-facade-double-workspace");
    let bin_dir = root.join(".bin");
    let provider_root = root.join("rust-provider");
    std::fs::create_dir_all(&provider_root).expect("create provider root");
    std::fs::write(
        provider_root.join("Cargo.toml"),
        "[package]\nname = \"rust-provider\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write manifest");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    install_state_home_provider(&root, "rust", &bin_dir.join("rs-harness"));
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "ingest",
            "items",
            "tests",
            "--workspace",
            "rust-provider",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search");

    assert!(!output.status.success(), "stdout={:?}", output.stdout);
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("expected at most one --workspace argument"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rust_search_facade_uses_explicit_workspace_for_graph_backend() {
    let root = temp_project_root("rust-search-facade-explicit-workspace");
    let bin_dir = root.join(".bin");
    let provider_root = root.join("rust-provider");
    std::fs::create_dir_all(&provider_root).expect("create provider root");
    std::fs::write(
        provider_root.join("Cargo.toml"),
        "[package]\nname = \"rust-provider\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write manifest");
    std::fs::create_dir_all(provider_root.join("src")).expect("create provider source root");
    std::fs::write(
        provider_root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub struct ProjectMarker;\n",
    )
    .expect("write provider search fixture");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    install_state_home_provider(&root, "rust", &bin_dir.join("rs-harness"));
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "source_index_fixture",
            "project_marker",
            "--workspace",
            "rust-provider",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-frontier]"), "{stdout}");
    assert!(
        stdout.contains(
            &std::fs::canonicalize(provider_root.join("src/lib.rs"))
                .expect("canonical provider source")
                .display()
                .to_string()
        ),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn check_facade_uses_explicit_workspace_directory() {
    let root = temp_project_root("check-facade-positional-directory-root");
    let bin_dir = root.join(".bin");
    let provider_root = root.join("fixture");
    std::fs::create_dir_all(provider_root.join("src")).expect("create provider root");
    write_pwd_provider(&bin_dir, "gslph");
    install_state_home_provider(&root, "gerbil-scheme", &bin_dir.join("gslph"));
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["gerbil-scheme", "check", "--workspace", "fixture"])
        .output()
        .expect("run asp gerbil-scheme check");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        format!(
            "{}\n",
            std::fs::canonicalize(&provider_root)
                .expect("canonical provider root")
                .display()
        )
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_query_facade_allows_explicit_workspace_outside_activation_workspace() {
    let root = temp_project_root("gerbil-query-facade-workspace-boundary");
    let bin_dir = root.join(".bin");
    let outside_root = root.parent().expect("temp root parent").join(format!(
        "{}-outside",
        root.file_name()
            .and_then(|name| name.to_str())
            .expect("temp root name")
    ));
    std::fs::create_dir_all(&outside_root).expect("create outside root");
    std::fs::write(outside_root.join("build.ss"), ";; outside build\n")
        .expect("write outside build file");
    write_pwd_provider(&bin_dir, "gslph");
    install_state_home_provider(&root, "gerbil-scheme", &bin_dir.join("gslph"));
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "query",
            "--selector",
            "build.ss:1-1",
            "--workspace",
            outside_root.to_str().expect("outside root utf8"),
            "--code",
        ])
        .output()
        .expect("run asp gerbil-scheme query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.trim_end().ends_with(
            outside_root
                .file_name()
                .expect("outside root file name")
                .to_str()
                .expect("outside root utf8")
        ),
        "{stdout}"
    );
    assert!(!stdout.contains(";; outside build"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside_root);
}

#[test]
fn rust_query_rebases_root_selector_to_workspace_member() {
    let root = temp_project_root("rust-query-workspace-member-selector");
    let member = root.join("languages/member");
    std::fs::create_dir_all(member.join("src")).expect("create member source");
    std::fs::write(
        member.join("Cargo.toml"),
        "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write member manifest");
    std::fs::write(member.join("src/lib.rs"), "pub fn value() -> u8 { 1 }\n")
        .expect("write member source");
    let rebased_selector = "rust://src/lib.rs#item/function/value";
    install_exact_selector_projection_provider(
        &root,
        &member,
        rebased_selector,
        b"rebased-selector-ok\n",
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--selector",
            "rust://languages/member/src/lib.rs#item/function/value",
            "--workspace",
            root.to_str().expect("root utf8"),
            "--code",
        ])
        .output()
        .expect("run asp rust query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "rebased-selector-ok\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rust_query_runs_workspace_member_selector_from_member_root() {
    let root = temp_project_root("rust-query-workspace-member-cwd");
    let member = root.join("languages/member");
    std::fs::create_dir_all(member.join("src")).expect("create member source");
    std::fs::write(
        member.join("Cargo.toml"),
        "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write member manifest");
    std::fs::write(member.join("src/lib.rs"), "pub fn value() -> u8 { 1 }\n")
        .expect("write member source");
    let projection = format!("{}\n", member.display());
    install_exact_selector_projection_provider(
        &root,
        &member,
        "rust://src/lib.rs#item/function/value",
        projection.as_bytes(),
    );

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--selector",
            "rust://languages/member/src/lib.rs#item/function/value",
            "--workspace",
            root.to_str().expect("root utf8"),
            "--code",
        ])
        .output()
        .expect("run asp rust query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(std::path::Path::new(stdout.trim()), member);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rust_search_facade_does_not_treat_positional_path_as_project_root() {
    let root = temp_project_root("rust-search-facade-positional-workspace-boundary");
    let bin_dir = root.join(".bin");
    let outside_root = root.parent().expect("temp root parent").join(format!(
        "{}-outside",
        root.file_name()
            .and_then(|name| name.to_str())
            .expect("temp root name")
    ));
    std::fs::create_dir_all(&outside_root).expect("create outside root");
    std::fs::write(
        outside_root.join("Cargo.toml"),
        "[package]\nname = \"outside\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write outside manifest");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    install_state_home_provider(&root, "rust", &bin_dir.join("rs-harness"));
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "source_index_fixture",
            "outside",
            outside_root.to_str().expect("outside root utf8"),
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-frontier]"), "{stdout}");
    assert!(stdout.contains(&outside_root.display().to_string()), "{stdout}");
    let outside_activation =
        agent_semantic_runtime::project_state_paths_with_state_home(&outside_root, state_home(&root))
            .expect("outside ephemeral State Home paths")
            .activation_path;
    assert!(
        !outside_activation.exists(),
        "positional search scope must not materialize an activation for {}",
        outside_root.display()
    );
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside_root);
}

fn install_exact_selector_projection_provider(
    root: &Path,
    member: &Path,
    structural_selector: &str,
    projection: &[u8],
) {
    let bin_dir = root.join(".bin");
    let packet_path = root.join("exact-selector-projection-packet.json");
    let provider_path = bin_dir.join("rs-harness");
    let delegate_path = bin_dir.join(".rs-harness-delegate");
    std::fs::create_dir_all(&bin_dir).expect("create exact-selector provider bin");
    std::fs::write(
        &delegate_path,
        format!(
            "#!/bin/sh\nparser_identity=''\nquery_pack_identity=''\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --asp-parser-identity-digest) parser_identity=\"$2\"; shift 2 ;;\n    --asp-query-pack-digest) query_pack_identity=\"$2\"; shift 2 ;;\n    *) shift ;;\n  esac\ndone\nsed -e \"s/__ASP_PARSER_IDENTITY__/$parser_identity/g\" -e \"s/__ASP_QUERY_PACK_IDENTITY__/$query_pack_identity/g\" \"{}\"\n",
            packet_path.display()
        ),
    )
    .expect("write exact-selector provider delegate");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&delegate_path)
            .expect("exact-selector provider delegate metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&delegate_path, permissions)
            .expect("exact-selector provider delegate permissions");
    }
    write_marker_provider(&bin_dir, "rs-harness", &bin_dir.join(".rs-harness-marker"));
    install_state_home_provider(root, "rust", &provider_path);
    let installed_delegate = state_runtime_bin(root).join(".rs-harness-delegate");
    std::fs::copy(&delegate_path, &installed_delegate)
        .expect("install exact-selector provider delegate in State Home");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&installed_delegate)
            .expect("installed exact-selector delegate metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&installed_delegate, permissions)
            .expect("installed exact-selector delegate permissions");
    }
    write_activation(root, &[provider("rust", Vec::new())]);

    let activation_path =
        agent_semantic_runtime::project_state_paths_with_state_home(root, state_home(root))
            .expect("State Home paths")
            .activation_path;
    let activation = std::fs::read_to_string(activation_path).expect("read activation");
    let activation =
        agent_semantic_hook::parse_hook_activation(&activation).expect("parse activation");
    let provider = activation
        .providers
        .iter()
        .find(|provider| provider.language_id == "rust")
        .expect("activated Rust provider");
    let parser_identity =
        agent_semantic_content_identity::exact_selector_projection_packet::derive_parser_identity_digest_v1(
            &provider.provider_id.as_str().into(),
            &provider.execution_command_digest.as_str().into(),
            &provider.semantic_registry_digest.as_str().into(),
        );
    let query_pack_json =
        serde_json::to_vec(&provider.query_pack_descriptor).expect("query pack JSON");
    let query_pack_identity =
        agent_semantic_content_identity::exact_selector_projection_packet::derive_query_pack_identity_digest_v1(
            &query_pack_json,
        );
    let source = std::fs::read(member.join("src/lib.rs")).expect("read exact-selector source");
    let canonical_item_selector =
        agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelectorV1::new(
            agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentityV1::new(
                "rust", "function", "value",
            ),
            structural_selector,
        );
    let language_id =
        agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketLanguageIdV1::from(
            "rust",
        );
    let provider_id =
        agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketProviderIdV1::from(
            provider.provider_id.as_str(),
        );
    let owner_path =
        agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketOwnerPathV1::from(
            "src/lib.rs",
        );
    let selector =
        agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketStructuralSelectorV1::from(
            structural_selector,
        );
    let packet =
        agent_semantic_content_identity::exact_selector_projection_packet::build_exact_selector_projection_packet_v1(
            agent_semantic_content_identity::exact_selector_projection_packet::ExactSelectorProjectionPacketV1Input {
                language_id: &language_id,
                provider_id: &provider_id,
                canonical_item_selector,
                parser_identity_digest: &parser_identity,
                query_pack_digest: &query_pack_identity,
                owner_path: &owner_path,
                structural_selector: &selector,
                projection_mode:
                    agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Code,
                source: &source,
                normalized_parser_facts: br#"{"kind":"function","name":"value"}"#,
                projection,
            },
        );
    let packet_json = serde_json::to_string(&packet)
        .expect("serialize exact-selector packet")
        .replace(parser_identity.as_str(), "__ASP_PARSER_IDENTITY__")
        .replace(query_pack_identity.as_str(), "__ASP_QUERY_PACK_IDENTITY__");
    std::fs::write(packet_path, packet_json).expect("write exact-selector packet");
}
