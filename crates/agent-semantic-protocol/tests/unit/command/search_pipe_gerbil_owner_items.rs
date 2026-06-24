#[allow(dead_code)]
#[path = "../../../src/command/search_pipe_gerbil_owner_items.rs"]
mod search_pipe_gerbil_owner_items;

use std::time::{Duration, Instant};

use search_pipe_gerbil_owner_items::{
    collect_gerbil_owner_items, render_inline_gerbil_owner_items,
};

#[test]
fn gerbil_owner_items_large_source_stays_inline_and_filters_short_noise() {
    let mut source =
        "(export compile-module)\n(def (compile-module ctx mod) (invoke-gsc mod))\n(C \"-keep-scm\")\n(f mod)\n".to_string();
    for index in 0..2500 {
        source.push_str(&format!(
            "(def (helper-{index} value) (compile-file value) (compile-scm-file value))\n"
        ));
    }

    let mut fastest = None;
    let mut output = String::new();
    for _ in 0..5 {
        let started_at = Instant::now();
        let items = collect_gerbil_owner_items(&source);
        let sample_output = render_inline_gerbil_owner_items(
            "src/gerbil/compiler/driver.ss",
            "compile-module|invoke-gsc|parallel|compile-file|compile-scm-file|gsc-options|keep-scm",
            &items,
        );
        let elapsed = started_at.elapsed();
        if fastest.is_none_or(|best_elapsed| elapsed < best_elapsed) {
            fastest = Some(elapsed);
            output = sample_output;
        }
        if elapsed < Duration::from_millis(100) {
            break;
        }
    }
    let elapsed = fastest.expect("Gerbil owner-items timing sample");

    assert!(
        elapsed < Duration::from_millis(100),
        "Gerbil owner-items inline parse/render exceeded 100ms: {elapsed:?}"
    );
    assert!(
        output.contains("I=item:symbol(compile-module)@src/gerbil/compiler/driver.ss:2:2!syntax"),
        "{output}"
    );
    assert!(
        output.contains("item:symbol(invoke-gsc)@src/gerbil/compiler/driver.ss:2:2!syntax"),
        "{output}"
    );
    assert!(
        output.contains(
            "nextCommand=asp gerbil-scheme query --selector src/gerbil/compiler/driver.ss:2:2 --workspace . --code"
        ),
        "{output}"
    );
    assert!(
        !output.contains("item:symbol(C)@")
            && !output.contains("item:symbol(f)@")
            && output.contains("reason=rust-inline-gerbil-owner-items"),
        "{output}"
    );
}
