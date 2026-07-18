use super::render_dynamic_owner_items;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn warm_rust_owner_render_is_millisecond_scale_and_single_receipt() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-owner-items-latency-{}-{nonce}",
        std::process::id()
    ));
    let owner = Path::new("src/lib.rs");
    fs::create_dir_all(root.join("src")).expect("create fixture source dir");
    fs::write(
        root.join(owner),
        "pub fn target_symbol() -> usize { 1 }\nfn helper_symbol() {}\n",
    )
    .expect("write fixture owner");

    let started = Instant::now();
    for _ in 0..64 {
        let output =
            render_dynamic_owner_items("rust", &root, &root, owner, "target_symbol", "seeds")
                .expect("render owner items")
                .expect("Rust owner items output");
        assert_eq!(output.matches("[search-owner]").count(), 1, "{output}");
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(1_000),
        "64 warm in-process owner renders exceeded the millisecond-scale budget: {elapsed:?}"
    );
    let _ = fs::remove_dir_all(root);
}
