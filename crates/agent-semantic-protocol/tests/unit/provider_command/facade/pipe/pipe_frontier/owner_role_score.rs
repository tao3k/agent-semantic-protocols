use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn source_owner_beats_test_corpus_when_query_has_no_test_intent() {
    let root = temp_project_root("search-pipe-owner-role-source");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/compiler")).expect("create source dir");
    std::fs::create_dir_all(root.join("tests/cases/projects/privacyCheck-IndirectReference"))
        .expect("create test corpus dir");
    std::fs::write(
        root.join("src/compiler/moduleResolution.ts"),
        "export function resolveModuleName() { return 'compiler module resolution'; }\n",
    )
    .expect("write source owner");
    std::fs::write(
        root.join("tests/cases/projects/privacyCheck-IndirectReference/indirectExternalModule.ts"),
        "compiler trace module resolution project references compiler trace module resolution\n",
    )
    .expect("write test corpus owner");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "pipe",
            "compiler trace module resolution project references",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp typescript search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("nextCommand=asp typescript search owner src/compiler/moduleResolution.ts"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("nextCommand=asp typescript search owner tests/cases/"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_owner_beats_unittests_corpus_when_query_has_no_test_intent() {
    let root = temp_project_root("search-pipe-owner-role-unittests");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/compiler")).expect("create source dir");
    std::fs::create_dir_all(root.join("src/testRunner/unittests/tsbuild"))
        .expect("create unittests dir");
    std::fs::write(
        root.join("src/compiler/moduleNameResolver.ts"),
        "export function resolveModuleName() { return 'compiler module resolution'; }\n",
    )
    .expect("write source owner");
    std::fs::write(
        root.join("src/testRunner/unittests/tsbuild/inferredTypeFromTransitiveModule.ts"),
        "compiler trace module resolution project references compiler trace module resolution\n",
    )
    .expect("write unittests owner");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "pipe",
            "compiler trace module resolution project references",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp typescript search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout
            .contains("nextCommand=asp typescript search owner src/compiler/moduleNameResolver.ts"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("nextCommand=asp typescript search owner src/testRunner/unittests/"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn test_intent_keeps_test_corpus_owner_eligible() {
    let root = temp_project_root("search-pipe-owner-role-tests");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/compiler")).expect("create source dir");
    std::fs::create_dir_all(root.join("tests/cases/projects/privacyCheck-IndirectReference"))
        .expect("create test corpus dir");
    std::fs::write(
        root.join("src/compiler/moduleResolution.ts"),
        "export function resolveModuleName() { return 'compiler module resolution'; }\n",
    )
    .expect("write source owner");
    std::fs::write(
        root.join("tests/cases/projects/privacyCheck-IndirectReference/indirectExternalModule.ts"),
        "compiler trace module resolution project references tests cases compiler trace module resolution\n",
    )
    .expect("write test corpus owner");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "pipe",
            "compiler trace module resolution project references tests cases",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp typescript search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("nextCommand=asp typescript search owner tests/cases/projects/privacyCheck-IndirectReference/indirectExternalModule.ts"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn low_cohesion_secondary_artifact_does_not_fallback_to_owner_items_first() {
    let root = temp_project_root("search-pipe-owner-role-template-gate");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    for path in [
        "packages/create-vite/template-react-ts",
        "packages/create-vite/template-vue",
        "packages/plugin-retired/template",
        "packages/vite/templates/server",
    ] {
        std::fs::create_dir_all(root.join(path)).expect("create package dir");
    }
    std::fs::write(
        root.join("packages/create-vite/template-react-ts/eslint.config.js"),
        "vite config resolution plugin container ordering\n",
    )
    .expect("write template owner");
    std::fs::write(
        root.join("packages/create-vite/template-vue/eslint.config.js"),
        "vite config\n",
    )
    .expect("write second template owner");
    std::fs::write(
        root.join("packages/plugin-retired/template/index.ts"),
        "export const plugin = true\n",
    )
    .expect("write plugin owner");
    std::fs::write(
        root.join("packages/vite/templates/server/server.ts"),
        "export const server = true\n",
    )
    .expect("write source owner");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "pipe",
            "connect config resolution plugin container ordering",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp typescript search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("packageCohesion=low"), "{stdout}");
    assert!(
        stdout.contains(
            "ownerCoverage=bestOwner=packages/create-vite/template-react-ts/eslint.config.js"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("A1=owner-items("), "{stdout}");
    assert!(
        !stdout.contains("recommendedNext=A1.owner-items"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn low_cohesion_source_owner_with_weak_axis_coverage_does_not_fallback_first() {
    let root = temp_project_root("search-pipe-owner-role-weak-axis");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    for path in [
        "packages/vite/src/node/server",
        "packages/create-vite/src",
        "packages/plugin-retired/src",
        "packages/dashboard/src",
    ] {
        std::fs::create_dir_all(root.join(path)).expect("create package dir");
    }
    std::fs::write(
        root.join("packages/vite/src/node/server/pluginContainer.ts"),
        "export const pluginContainer = true\n",
    )
    .expect("write weak source owner");
    std::fs::write(
        root.join("packages/create-vite/src/index.ts"),
        "export const createVite = true\n",
    )
    .expect("write drift owner");
    std::fs::write(
        root.join("packages/plugin-retired/src/index.ts"),
        "export const plugin = true\n",
    )
    .expect("write plugin owner");
    std::fs::write(
        root.join("packages/dashboard/src/server.ts"),
        "export const server = true\n",
    )
    .expect("write server owner");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "pipe",
            "connect config resolution plugin container ordering",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp typescript search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("packageCohesion=low"), "{stdout}");
    assert!(!stdout.contains("A1=owner-items("), "{stdout}");
    assert!(
        !stdout.contains("recommendedNext=A1.owner-items"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
