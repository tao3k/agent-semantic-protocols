use agent_semantic_client_core::{
    ClientMethod, ClientRequest, LanguageId, ProviderRegistrySnapshot,
};

use super::{python_provider, rust_provider, temp_root};
use crate::cache_cli::writeback::write_prompt_output_cache_after_provider_success;

#[test]
fn owner_items_search_writeback_replays_prompt_output_artifact() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("owner-items-search-writeback");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join("src/export")).expect("create source dir");
    std::fs::write(root.join("src/export/event.rs"), "pub enum Event {}\n").expect("write source");
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Search, &root)
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "owner".to_string(),
            "src/export/event.rs".to_string(),
            "items".to_string(),
            "--query".to_string(),
            "Event".to_string(),
        ]);
    let stdout = "[search-owner] q=src/export/event.rs pkg=. own=1 item=1 itemQuery=Event\n\
|owner src/export/event.rs role=source source=parser-visible-module lines=1 imports=0\n\
|query itemQuery=Event status=hit match=exact item=1 reason=parser-item-exact next=query-code\n\
|item Event kind=enum public=true doc=false next=symbol:Event read=src/export/event.rs:1:1 syn=enum_item/name\n";

    let probe = write_prompt_output_cache_after_provider_success(
        &root,
        &snapshot,
        &request,
        stdout.as_bytes(),
        &[],
    )
    .expect("writeback probe");
    let replay = probe.replay.expect("owner output replay");

    assert_eq!(replay.stdout, stdout.as_bytes());
    assert_eq!(probe.sqlite_write_count, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn query_selector_code_writeback_replays_prompt_output_artifact() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("query-selector-code-writeback");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join("tests/unit")).expect("create test dir");
    std::fs::write(
        root.join("tests/unit/test_query_packet.py"),
        "def test_blocks():\n    contentBlocks = []\n    assert contentBlocks == []\n",
    )
    .expect("write source");
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![python_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Query, &root)
        .with_language(LanguageId::from("python"))
        .with_forwarded_args(vec![
            "--selector".to_string(),
            "tests/unit/test_query_packet.py:1-3".to_string(),
            "--term".to_string(),
            "contentBlocks".to_string(),
            "--code".to_string(),
            ".".to_string(),
        ]);
    let stdout = "def test_blocks():\n    contentBlocks = []\n    assert contentBlocks == []\n";

    let probe = write_prompt_output_cache_after_provider_success(
        &root,
        &snapshot,
        &request,
        stdout.as_bytes(),
        &[],
    )
    .expect("writeback probe");
    let replay = probe.replay.expect("query code output replay");

    assert_eq!(replay.stdout, stdout.as_bytes());
    assert_eq!(probe.sqlite_write_count, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn prime_seed_prompt_output_writeback_adds_search_output_replay_artifact() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("prime-seed-prompt-output-search-output");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join("src")).expect("create source dir");
    std::fs::write(root.join("src/lib.rs"), "pub fn cached_prime() {}\n").expect("write source");
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Search, &root)
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "prime".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            ".".to_string(),
        ]);
    let stdout = "[search-prime] root=. alg=fast-prime-frontier-v1\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases: graph:{G=search,O=owner}\n\
O=owner:path(src/lib.rs)!owner\n\
G>{O:selects}\n\
rank=O frontier=O.owner\n";

    let probe = write_prompt_output_cache_after_provider_success(
        &root,
        &snapshot,
        &request,
        stdout.as_bytes(),
        &[],
    )
    .expect("writeback probe");
    let replay = probe.replay.expect("search output replay");

    assert_eq!(replay.stdout, stdout.as_bytes());
    assert_eq!(probe.sqlite_write_count, 2);
    let _ = std::fs::remove_dir_all(root);
}
