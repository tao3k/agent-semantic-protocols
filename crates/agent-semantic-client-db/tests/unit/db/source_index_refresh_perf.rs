use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use agent_semantic_client_core::CacheGenerationId;
use agent_semantic_client_core::ClientCacheFileHash;
use agent_semantic_client_core::LanguageId;
use agent_semantic_client_core::ProviderId;
use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_client_db::ClientDbSourceIndexImport;
use agent_semantic_client_db::ClientDbSourceIndexLookupState;
use agent_semantic_client_db::ClientDbSourceIndexOwner;
use agent_semantic_client_db::ClientDbSourceIndexRefreshRequest;
use agent_semantic_client_db::ClientDbSourceIndexSelector;

const SOURCE_INDEX_WARM_REUSE_GATE: Duration = Duration::from_millis(750);
const SOURCE_INDEX_HASH_REUSE_GATE: Duration = Duration::from_millis(25);
#[cfg(not(debug_assertions))]
const SOURCE_INDEX_1193_OWNER_COLD_WRITE_GATE: Duration = Duration::from_secs(1);
#[cfg(not(debug_assertions))]
const SOURCE_INDEX_1193_OWNER_COLD_WRITE_DISK_GATE_BYTES: u64 = 16 * 1024 * 1024;
#[cfg(not(debug_assertions))]
const SOURCE_INDEX_1193_OWNER_COLD_WRITE_RSS_GROWTH_GATE_BYTES: u64 = 64 * 1024 * 1024;
#[cfg(not(debug_assertions))]
const SOURCE_INDEX_POSTING_FRONTIER_COLD_WRITE_GATE: Duration = Duration::from_secs(1);
#[cfg(not(debug_assertions))]
const SOURCE_INDEX_1193_OWNER_ONE_PERCENT_REFRESH_GATE: Duration = Duration::from_millis(500);
#[cfg(not(debug_assertions))]
const SOURCE_INDEX_1193_OWNER_HIGH_FANOUT_COLD_LOOKUP_GATE: Duration = Duration::from_millis(400);

static SOURCE_INDEX_REFRESH_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[path = "source_index_refresh_perf_identity.rs"]
mod source_index_refresh_perf_identity;

#[path = "source_index_refresh_perf_incremental.rs"]
mod source_index_refresh_perf_incremental;

async fn source_index_refresh_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
    SOURCE_INDEX_REFRESH_TEST_LOCK.lock().await
}

#[cfg(not(debug_assertions))]
#[tokio::test(flavor = "current_thread")]
async fn source_index_1193_owner_cold_write_stays_inside_v1_gate() {
    let _test_guard = source_index_refresh_test_guard().await;
    let root = temp_project_root("source-index-1193-owner-cold-write");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let rss_before = current_process_rss_bytes().expect("measure RSS before cold write");
    let started_at = Instant::now();
    let refresh = commit_fixture_generation(&fixture, large_refresh_request(&project_root, 1_193))
        .expect("write 1193-owner source-index snapshot");
    let elapsed = started_at.elapsed();
    let rss_after = current_process_rss_bytes().expect("measure RSS after cold write");
    let rss_growth_bytes = rss_after.saturating_sub(rss_before);
    let disk_bytes = directory_tree_bytes(&client_dir).expect("measure cold-write disk bytes");

    assert!(!refresh.reused_generation, "cold import must publish rows");
    assert_eq!(refresh.owner_count, 1_193);
    assert_eq!(refresh.selector_count, 1_193);
    assert_eq!(refresh.changed_owner_count, 1_193);
    assert_eq!(refresh.removed_owner_count, 0);
    assert_eq!(
        refresh.posting_write_count, 5_243,
        "cold-write receipt must match the deterministic canonical posting frontier"
    );
    assert!(
        elapsed < SOURCE_INDEX_1193_OWNER_COLD_WRITE_GATE,
        "1193-owner source-index cold write exceeded V1 DB gate: elapsed={elapsed:?} gate={SOURCE_INDEX_1193_OWNER_COLD_WRITE_GATE:?}"
    );
    assert!(
        disk_bytes <= SOURCE_INDEX_1193_OWNER_COLD_WRITE_DISK_GATE_BYTES,
        "1193-owner source-index cold write exceeded disk gate: diskBytes={disk_bytes} gateBytes={SOURCE_INDEX_1193_OWNER_COLD_WRITE_DISK_GATE_BYTES}"
    );
    assert!(
        rss_growth_bytes <= SOURCE_INDEX_1193_OWNER_COLD_WRITE_RSS_GROWTH_GATE_BYTES,
        "1193-owner source-index cold write exceeded RSS growth gate: rssGrowthBytes={rss_growth_bytes} gateBytes={SOURCE_INDEX_1193_OWNER_COLD_WRITE_RSS_GROWTH_GATE_BYTES}"
    );
    eprintln!(
        "source-index-1193-owner-cold-write elapsedMicros={} diskBytes={disk_bytes} rssGrowthBytes={rss_growth_bytes}",
        elapsed.as_micros()
    );

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(debug_assertions))]
#[tokio::test(flavor = "current_thread")]
async fn source_index_1278_owner_posting_frontier_cold_write_stays_inside_v1_gate() {
    let _test_guard = source_index_refresh_test_guard().await;
    let root = temp_project_root("source-index-1278-owner-posting-frontier-cold-write");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let request = large_high_term_refresh_request(&project_root, 1_278, 32);
    let started_at = Instant::now();
    let refresh = commit_fixture_generation(&fixture, request)
        .expect("write bounded-posting source-index snapshot");
    let elapsed = started_at.elapsed();

    assert_eq!(refresh.owner_count, 1_278);
    assert_eq!(refresh.selector_count, 1_278);
    assert!(
        (12_000..=20_448).contains(&refresh.posting_write_count),
        "posting frontier must remain large enough to exercise bulk writes while retaining at most 16 ranked terms per owner: refresh={refresh:?}"
    );
    assert!(
        elapsed < SOURCE_INDEX_POSTING_FRONTIER_COLD_WRITE_GATE,
        "posting-frontier source-index cold write exceeded V1 DB gate: elapsed={elapsed:?} gate={SOURCE_INDEX_POSTING_FRONTIER_COLD_WRITE_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(debug_assertions))]
#[tokio::test(flavor = "current_thread")]
async fn source_index_1193_owner_one_percent_refresh_stays_inside_v1_gate() {
    let _test_guard = source_index_refresh_test_guard().await;
    let root = temp_project_root("source-index-1193-owner-one-percent-refresh");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    commit_fixture_generation(&fixture, large_refresh_request(&project_root, 1_193))
        .expect("write initial 1193-owner source-index snapshot");

    let mut changed_request = large_refresh_request(&project_root, 1_193);
    for index in 0..12 {
        let owner_path = format!("src/generated/owner_{index}.rs");
        let symbol = format!("source_index_large_owner_{index}");
        let selector_id = format!("rust://{owner_path}#item/function/{symbol}");
        let changed_source = format!("pub fn {symbol}() {{ let changed = {index}; }}");
        let file_hash = &mut changed_request.import.file_hashes[index];
        file_hash.sha256 = format!(
            "{:x}",
            <sha2::Sha256 as sha2::Digest>::digest(changed_source.as_bytes())
        );
        file_hash.byte_len = changed_source.len() as u64;
        let selector = &mut changed_request.import.selectors[index];
        selector.source = changed_source.clone().into();
        selector.projection_record = crate::projection_fixture::projection_record(
            crate::projection_fixture::ProjectionFixtureInput {
                language_id: "rust",
                provider_id: "asp-rust",
                owner_path: &owner_path,
                structural_selector: &selector_id,
                item_kind: "function",
                item_name: &symbol,
                source: changed_source.as_bytes(),
                source_byte_start: 0,
                source_byte_end: changed_source.len() as u64,
            },
        );
    }
    changed_request.source_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
            changed_request
                .import
                .file_hashes
                .iter()
                .map(|file_hash| (file_hash.path.clone(), file_hash.sha256.clone())),
        )
        .evidence(
            agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
            "d".repeat(64),
        );
    changed_request.import.owners.truncate(12);
    changed_request.import.selectors.truncate(12);

    let started_at = Instant::now();
    let refresh = commit_fixture_generation(&fixture, changed_request)
        .expect("refresh one-percent changed source-index snapshot");
    let elapsed = started_at.elapsed();

    assert!(
        !refresh.reused_generation,
        "changed import must publish rows"
    );
    assert_eq!(refresh.owner_count, 1_193);
    assert_eq!(refresh.selector_count, 1_193);
    assert_eq!(
        refresh.changed_owner_count, 12,
        "the one-percent scenario must serialize only its 12 changed owners"
    );
    assert_eq!(refresh.removed_owner_count, 0);
    assert_eq!(
        refresh.posting_write_count, 204,
        "12 changed owners with 17 canonical terms each must rebuild only 204 postings"
    );
    assert!(
        elapsed < SOURCE_INDEX_1193_OWNER_ONE_PERCENT_REFRESH_GATE,
        "1193-owner one-percent refresh exceeded V1 DB gate: elapsed={elapsed:?} gate={SOURCE_INDEX_1193_OWNER_ONE_PERCENT_REFRESH_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(debug_assertions))]
#[tokio::test(flavor = "current_thread")]
async fn source_index_1193_owner_high_fanout_lookup_stays_inside_v1_gate() {
    let _test_guard = source_index_refresh_test_guard().await;
    let root = temp_project_root("source-index-1193-owner-high-fanout-lookup");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let refresh_request = large_refresh_request(&project_root, 1_193);
    let source_snapshot = refresh_request.source_snapshot.clone();
    commit_fixture_generation(&fixture, refresh_request)
        .expect("write high-fanout source-index snapshot");

    let cold_started_at = Instant::now();
    let cold_lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
        "source",
        Some(&LanguageId::from("rust")),
        128,
    )
    .await
    .expect("read high-fanout relational postings");
    let cold_elapsed = cold_started_at.elapsed();

    assert!(
        cold_lookup.candidates.len() >= 128,
        "high-fanout lookup must return the bounded owner window: lookup={cold_lookup:?}"
    );
    assert!(
        cold_elapsed < SOURCE_INDEX_1193_OWNER_HIGH_FANOUT_COLD_LOOKUP_GATE,
        "1193-owner high-fanout cold lookup exceeded V1 DB gate: elapsed={cold_elapsed:?} gate={SOURCE_INDEX_1193_OWNER_HIGH_FANOUT_COLD_LOOKUP_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "current_thread")]
async fn source_index_refresh_reuse_stays_on_structured_turso_path() {
    let _test_guard = source_index_refresh_test_guard();
    let root = temp_project_root("source-index-refresh-reuse-perf");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let request = refresh_request(&project_root);
    let first =
        commit_fixture_generation(&fixture, request.clone()).expect("cold source-index refresh");
    assert!(!first.reused_generation, "first refresh should write rows");

    let started_at = Instant::now();
    let second = commit_fixture_generation(&fixture, request).expect("warm source-index refresh");
    let elapsed = started_at.elapsed();

    assert!(second.reused_generation, "second refresh should reuse rows");
    assert_eq!(second.owner_count, 1);
    assert_eq!(second.selector_count, 1);
    assert!(
        elapsed < SOURCE_INDEX_WARM_REUSE_GATE,
        "warm source-index refresh should stay on structured Turso path; elapsed={elapsed:?} gate={SOURCE_INDEX_WARM_REUSE_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

fn refresh_request(project_root: &Path) -> ClientDbSourceIndexRefreshRequest {
    let structural_selector =
        "rust://src/source_index_perf.rs#item/function/source_index_perf_fixture";
    let source = b"pub fn source_index_perf_fixture() {}";
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
        "src/source_index_perf.rs",
        blake3::hash(source).to_hex().to_string(),
    )])
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        crate::snapshot_fixture::source_snapshot_evidence().provider_digest,
    );
    let generation_id = agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
        &source_snapshot,
    );
    ClientDbSourceIndexRefreshRequest {
        file_count: 1,
        source_snapshot,
        import: ClientDbSourceIndexImport {
            source_blobs: agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
                [(
                    agent_semantic_client_db::ClientDbSourceIndexPath::from(
                        "src/source_index_perf.rs",
                    ),
                    source.to_vec(),
                )],
            ),
            relations: Vec::new(),
            generation_id,
            project_root: project_root.to_path_buf(),
            schema_id: "agent-semantic-client-db.source-index".to_string().into(),
            schema_version: "1".into(),
            file_hashes: vec![ClientCacheFileHash {
                path: "src/source_index_perf.rs".to_string(),
                sha256: format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(source)),
                byte_len: source.len() as u64,
                mtime_ms: 1,
            }],
            owners: vec![ClientDbSourceIndexOwner {
                owner_path: "src/source_index_perf.rs".into(),
                language_id: Some(LanguageId::from("rust")),
                provider_id: Some(ProviderId::from("asp-rust")),
                source_kind: "source".to_string().into(),
                line_count: Some(8),
                query_keys: vec!["source_index_perf_fixture".to_string().into()],
            }],
            selectors: vec![ClientDbSourceIndexSelector {
                owner_path: "src/source_index_perf.rs".into(),
                provider_id: ProviderId::from("asp-rust"),
                selector_id: structural_selector.into(),
                symbol: Some("source_index_perf_fixture".into()),
                kind: Some("function".into()),
                source: String::from_utf8(source.to_vec())
                    .expect("source-index fixture UTF-8")
                    .into(),
                query_keys: vec!["source_index_perf_fixture".to_string().into()],
                derived_projections: Vec::new(),
                projection_record: crate::projection_fixture::projection_record(
                    crate::projection_fixture::ProjectionFixtureInput {
                        language_id: "rust",
                        provider_id: "asp-rust",
                        owner_path: "src/source_index_perf.rs",
                        structural_selector,
                        item_kind: "function",
                        item_name: "source_index_perf_fixture",
                        source,
                        source_byte_start: 0,
                        source_byte_end: source.len() as u64,
                    },
                ),
            }],
        },
    }
}

fn commit_fixture_generation(
    fixture: &agent_semantic_client_db::fixture::SourceIndexFixture,
    mut request: ClientDbSourceIndexRefreshRequest,
) -> Result<agent_semantic_client_db::ClientDbSourceIndexRefreshReport, String> {
    let source_blobs = agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
        request.import.selectors.iter().map(|selector| {
            (
                selector
                    .projection_record
                    .proof
                    .owner_path()
                    .to_owned()
                    .into(),
                selector.projection_record.projection_payload.clone(),
            )
        }),
    );
    request.import.source_blobs = source_blobs.clone();
    fixture.commit_source_index_generation(request, &source_blobs)
}

fn large_refresh_request(
    project_root: &Path,
    owner_count: u32,
) -> ClientDbSourceIndexRefreshRequest {
    let mut file_hashes = Vec::with_capacity(owner_count as usize);
    let mut source_blob_entries = Vec::with_capacity(owner_count as usize);
    let mut owners = Vec::with_capacity(owner_count as usize);
    let mut selectors = Vec::with_capacity(owner_count as usize);
    for index in 0..owner_count {
        let owner_path = format!("src/generated/owner_{index}.rs");
        let symbol = format!("source_index_large_owner_{index}");
        let selector_id = format!("rust://{owner_path}#item/function/{symbol}");
        let source = format!("pub fn {symbol}() {{}}\n");
        file_hashes.push(ClientCacheFileHash {
            path: owner_path.clone(),
            sha256: format!(
                "{:x}",
                <sha2::Sha256 as sha2::Digest>::digest(source.as_bytes())
            ),
            byte_len: source.len() as u64,
            mtime_ms: u64::from(index) + 1,
        });
        source_blob_entries.push((
            agent_semantic_client_db::ClientDbSourceIndexPath::new(owner_path.clone()),
            source.as_bytes().to_vec(),
        ));
        owners.push(ClientDbSourceIndexOwner {
            owner_path: owner_path.clone().into(),
            language_id: Some(LanguageId::from("rust")),
            provider_id: Some(ProviderId::from("asp-rust")),
            source_kind: "source".to_string().into(),
            line_count: Some(1),
            query_keys: vec![symbol.clone().into()],
        });
        selectors.push(ClientDbSourceIndexSelector {
            owner_path: owner_path.into(),
            provider_id: ProviderId::from("asp-rust"),
            selector_id: selector_id.clone().into(),
            symbol: Some(symbol.clone().into()),
            kind: Some("function".into()),
            source: source.clone().into(),
            query_keys: vec![symbol.into()],
            derived_projections: Vec::new(),
            projection_record: crate::projection_fixture::projection_record(
                crate::projection_fixture::ProjectionFixtureInput {
                    language_id: "rust",
                    provider_id: "asp-rust",
                    owner_path: &format!("src/generated/owner_{index}.rs"),
                    structural_selector: &selector_id,
                    item_kind: "function",
                    item_name: &format!("source_index_large_owner_{index}"),
                    source: source.as_bytes(),
                    source_byte_start: 0,
                    source_byte_end: source.len() as u64,
                },
            ),
        });
    }
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        file_hashes
            .iter()
            .map(|file_hash| (file_hash.path.clone(), file_hash.sha256.clone())),
    )
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "d".repeat(64),
    );
    ClientDbSourceIndexRefreshRequest {
        file_count: owner_count,
        source_snapshot,
        import: ClientDbSourceIndexImport {
            source_blobs: agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
                source_blob_entries,
            ),
            relations: Vec::new(),
            generation_id: CacheGenerationId::from("snapshot-bound-source-index-large"),
            project_root: project_root.to_path_buf(),
            schema_id: "agent-semantic-client-db.source-index".to_string().into(),
            schema_version: "1".into(),
            file_hashes,
            owners,
            selectors,
        },
    }
}

#[cfg(not(debug_assertions))]
fn large_high_term_refresh_request(
    project_root: &Path,
    owner_count: u32,
    unique_terms_per_owner: u32,
) -> ClientDbSourceIndexRefreshRequest {
    let mut request = large_refresh_request(project_root, owner_count);
    for (owner_index, owner) in request.import.owners.iter_mut().enumerate() {
        owner.query_keys.extend(
            (0..unique_terms_per_owner)
                .map(|term_index| format!("z{owner_index:04}q{term_index:02}unique").into()),
        );
    }
    request
}

fn run_git(project_root: &Path, args: impl IntoIterator<Item = &'static str>) {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(args)
        .output()
        .expect("run git for source-index fixture");
    assert!(
        output.status.success(),
        "git source-index fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn temp_project_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-{label}-{nonce}"))
}

#[cfg(not(debug_assertions))]
fn directory_tree_bytes(root: &Path) -> Result<u64, String> {
    let mut bytes = 0_u64;
    for entry in std::fs::read_dir(root).map_err(|error| {
        format!(
            "read performance artifact directory {}: {error}",
            root.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "read performance artifact entry {}: {error}",
                root.display()
            )
        })?;
        let metadata = entry.metadata().map_err(|error| {
            format!(
                "read performance artifact metadata {}: {error}",
                entry.path().display()
            )
        })?;
        if metadata.is_dir() {
            bytes = bytes.saturating_add(directory_tree_bytes(&entry.path())?);
        } else if metadata.is_file() {
            bytes = bytes.saturating_add(metadata.len());
        }
    }
    Ok(bytes)
}

#[cfg(all(not(debug_assertions), target_os = "macos"))]
fn current_process_rss_bytes() -> Result<u64, String> {
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .map_err(|error| format!("measure current process RSS with ps: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "measure current process RSS with ps: status={}",
            output.status
        ));
    }
    let kibibytes = String::from_utf8(output.stdout)
        .map_err(|error| format!("decode current process RSS: {error}"))?
        .trim()
        .parse::<u64>()
        .map_err(|error| format!("parse current process RSS: {error}"))?;
    Ok(kibibytes.saturating_mul(1024))
}

#[cfg(all(not(debug_assertions), target_os = "linux"))]
fn current_process_rss_bytes() -> Result<u64, String> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|error| format!("read /proc/self/status for RSS: {error}"))?;
    let rss_line = status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .ok_or_else(|| "read /proc/self/status for RSS: VmRSS missing".to_string())?;
    let kibibytes = rss_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "read /proc/self/status for RSS: value missing".to_string())?
        .parse::<u64>()
        .map_err(|error| format!("parse /proc/self/status RSS: {error}"))?;
    Ok(kibibytes.saturating_mul(1024))
}

#[cfg(all(
    not(debug_assertions),
    not(any(target_os = "macos", target_os = "linux"))
))]
fn current_process_rss_bytes() -> Result<u64, String> {
    Err("source-index RSS performance gate supports macOS and Linux only".to_string())
}
