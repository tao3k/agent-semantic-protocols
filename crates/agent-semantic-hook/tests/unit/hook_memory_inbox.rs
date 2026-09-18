// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{HookMemoryInboxReader, append_workspace_mutation, compact_through, read_after};
use agent_semantic_client_protocol::HookWorkspaceMutationEvent;

#[test]
fn mmap_inbox_is_process_independent_ordered_and_incremental() {
    let root = tempfile::tempdir().expect("temporary inbox root");
    let path = root
        .path()
        .join("runtime/serving/mailboxes/hook-memory-inbox.v1.mmap");
    let resident_reader = HookMemoryInboxReader::open_or_create(&path).expect("resident reader");
    for (mutation_id, changed_path) in [("mutation-1", "src/a.rs"), ("mutation-2", "src/b.rs")] {
        append_workspace_mutation(
            &path,
            HookWorkspaceMutationEvent {
                mutation_id: mutation_id.to_owned(),
                project_root: root.path().display().to_string(),
                changed_paths: vec![changed_path.to_owned()],
                tool_name: "apply_patch".to_owned(),
                session_id: Some("session-1".to_owned()),
                tool_use_id: None,
            },
        )
        .expect("append mutation");
    }

    let all = resident_reader
        .read_after(0)
        .expect("read resident mapping");
    assert_eq!(
        all.iter()
            .map(|event| event.inbox_sequence)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    let incremental = resident_reader
        .read_after(1)
        .expect("read after acknowledgement");
    assert_eq!(incremental.len(), 1);
    assert_eq!(incremental[0].inbox_sequence, 2);
    assert_eq!(
        incremental[0]
            .workspace_mutation_event()
            .unwrap()
            .unwrap()
            .changed_paths,
        ["src/b.rs"]
    );

    assert!(compact_through(&path, 1).expect("compact acknowledged prefix") > 0);
    let retained = resident_reader.read_after(0).expect("read retained suffix");
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].inbox_sequence, 2);
    append_workspace_mutation(
        &path,
        HookWorkspaceMutationEvent {
            mutation_id: "mutation-3".to_owned(),
            project_root: root.path().display().to_string(),
            changed_paths: vec!["src/c.rs".to_owned()],
            tool_name: "apply_patch".to_owned(),
            session_id: None,
            tool_use_id: None,
        },
    )
    .expect("append after compaction");
    assert_eq!(
        read_after(&path, 0)
            .unwrap()
            .iter()
            .map(|event| event.inbox_sequence)
            .collect::<Vec<_>>(),
        [2, 3]
    );
}

#[test]
fn scenario_qualifies_bounded_append_work_without_runtime_or_workspace_scan() {
    let scenario =
        include_str!("../fixtures/scenarios/hook_memory_inbox_workspace_mutation/scenario.toml");
    let benchmark =
        include_str!("../fixtures/scenarios/hook_memory_inbox_workspace_mutation/benchmark.toml");
    assert!(scenario.contains("hook_runtime_socket_count = 0"));
    assert!(scenario.contains("hook_workspace_scan_count = 0"));
    assert!(scenario.contains("search_generation_build_count = 0"));
    assert!(benchmark.contains("workspace_owner_count = \"excluded\""));
    assert!(benchmark.contains("fsync_count = 0"));

    let root = tempfile::tempdir().expect("temporary inbox root");
    let path = root.path().join("hook-memory-inbox.v1.mmap");
    let mut elapsed = Vec::new();
    for sequence in 0..128 {
        let started = std::time::Instant::now();
        append_workspace_mutation(
            &path,
            HookWorkspaceMutationEvent {
                mutation_id: format!("mutation-{sequence}"),
                project_root: root.path().display().to_string(),
                changed_paths: vec![format!("src/owner-{sequence}.rs")],
                tool_name: "apply_patch".to_owned(),
                session_id: None,
                tool_use_id: None,
            },
        )
        .expect("bounded mmap append");
        elapsed.push(started.elapsed());
    }
    elapsed.sort_unstable();
    let p99 = elapsed[126];
    assert!(
        p99 < std::time::Duration::from_millis(100),
        "Hook mmap append p99 exceeded the V1 local-event budget: {p99:?}"
    );
}
