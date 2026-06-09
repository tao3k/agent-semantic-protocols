use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn typescript_owner_items_query_set_renders_item_selectors_without_provider() {
    let root = temp_project_root("search-owner-typescript-query-set");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("packages/effect/src")).expect("create source");
    std::fs::write(
        root.join("packages/effect/src/Fiber.ts"),
        "export interface Fiber {\n  readonly id: number\n}\n\nexport interface Queue {\n  readonly size: number\n}\n\nexport interface Stream {\n  readonly done: boolean\n}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "owner",
            "packages/effect/src/Fiber.ts",
            "items",
            "--query",
            "Fiber|Queue|Stream",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp typescript search owner items");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("I=item:symbol(Fiber)@packages/effect/src/Fiber.ts:1:3!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("I2=item:symbol(Queue)@packages/effect/src/Fiber.ts:5:7!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("I3=item:symbol(Stream)@packages/effect/src/Fiber.ts:9:11!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("syntax I selector=packages/effect/src/Fiber.ts:1:3 pattern='((interface_declaration name: (type_identifier) @interface.name) (#eq? @interface.name \"Fiber\"))'"),
        "{stdout}"
    );
    assert!(
        stdout.contains("frontier=Q.query,T.tests,O.owner,I.syntax,I2.syntax,I3.syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("recommendedNext=query-selector"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp typescript query --selector packages/effect/src/Fiber.ts:1:3 --workspace . --code"),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=owner-item-selector-ready"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("Fiber|Queue|Stream)@packages/effect/src/Fiber.ts"),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "TypeScript owner-items fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
