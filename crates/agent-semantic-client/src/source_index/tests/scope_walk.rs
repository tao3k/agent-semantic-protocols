use super::collect_non_symlink_scope_files;
use std::path::Path;

#[test]
fn fallback_scope_walk_is_deterministic_and_does_not_follow_symlinks() {
    let fixture = std::env::temp_dir().join(format!(
        "asp-source-index-symlink-cycle-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    let _ = std::fs::remove_dir_all(&fixture);
    std::fs::create_dir_all(fixture.join("nested")).expect("create nested fixture");
    std::fs::write(fixture.join("b.rs"), "fn b() {}").expect("write b.rs");
    std::fs::write(fixture.join("a.rs"), "fn a() {}").expect("write a.rs");
    std::fs::write(fixture.join("nested").join("c.rs"), "fn c() {}").expect("write nested c.rs");
    std::os::unix::fs::symlink(&fixture, fixture.join("cycle")).expect("create directory cycle");

    let files = collect_non_symlink_scope_files(
        &fixture,
        usize::MAX,
        |_| false,
        |path| path.extension().is_some_and(|extension| extension == "rs"),
    )
    .expect("collect fallback scope");
    let relative = files
        .iter()
        .map(|path| {
            path.strip_prefix(&fixture)
                .expect("fixture-relative path")
                .to_path_buf()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        relative,
        [
            Path::new("a.rs").to_path_buf(),
            Path::new("b.rs").to_path_buf(),
            Path::new("nested/c.rs").to_path_buf(),
        ]
    );
    std::fs::remove_dir_all(&fixture).expect("remove fixture");
}
