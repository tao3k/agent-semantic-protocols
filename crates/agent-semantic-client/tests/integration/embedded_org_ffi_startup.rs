// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Isolated process qualification for explicit embedded parser startup.

#[test]
fn explicit_startup_precedes_parallel_lossless_org_parsing() {
    // SAFETY: this binary contains one test; no application workers or children
    // are created before the explicit startup window finishes.
    unsafe { orgize::initialize_native_runtime() }.expect("embedded parser startup");
    let source = "* TODO Search closure\nUnicode: 搜索 λ\n";
    std::thread::scope(|scope| {
        let workers = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    let document = orgize::Org::try_parse(source).expect("FFI parse");
                    assert_eq!(document.to_org(), source);
                    assert_eq!(document.receipt().language, "org-mode");
                    assert!(document.receipt().parser_digest.is_some());
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().expect("parser worker");
        }
    });
}
