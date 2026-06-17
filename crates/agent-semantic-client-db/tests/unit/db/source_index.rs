use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId,
    SemanticSchemaVersion,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbSourceIndexImport, ClientDbSourceIndexLookup, ClientDbSourceIndexOwner,
    ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey, ClientDbSourceIndexSelector,
    ClientDbSourceIndexSource,
};

#[test]
fn source_index_replaces_and_reads_rust_owned_rows() {
    let root = temp_root("source-index");
    let db_path = root.join("client.sqlite3");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    let stats = db
        .replace_source_index(&source_index(&root, "source-main-1", "src/lib.ss"))
        .expect("replace source index");
    let summary = db.summary().expect("db summary");
    let report = ClientDb::inspect(&db_path);
    let owners = db
        .lookup_source_index_owners(&lookup(&root, "gerbil-poo"))
        .expect("lookup source owners");
    let latest_owners = db
        .latest_source_index_generation_owners(
            &root,
            &SemanticSchemaId::from("agent.semantic-protocols.semantic-source-index"),
            &SemanticSchemaVersion::from("1"),
        )
        .expect("latest source owners");

    assert_eq!(stats.owner_count, 1);
    assert_eq!(stats.selector_count, 1);
    assert_eq!(summary.source_index_generation_count, 1);
    assert_eq!(summary.source_index_owner_count, 1);
    assert_eq!(summary.source_index_selector_count, 1);
    assert_eq!(report.source_index_generation_count, 1);
    assert_eq!(report.source_index_owner_count, 1);
    assert_eq!(report.source_index_selector_count, 1);
    assert_eq!(owners.len(), 1);
    assert_eq!(latest_owners.len(), 1);
    assert_eq!(owners[0].owner_path.as_str(), "src/lib.ss");
    assert_eq!(latest_owners[0].owner_path.as_str(), "src/lib.ss");
    assert_eq!(
        owners[0].language_id.as_ref().map(LanguageId::as_str),
        Some("gerbil-scheme")
    );
    assert_eq!(
        owners[0].provider_id.as_ref().map(ProviderId::as_str),
        Some("rust-sql")
    );
    assert_eq!(owners[0].source_kind.as_str(), "scheme-source");
    assert_eq!(owners[0].line_count, Some(80));
    assert_eq!(
        owners[0]
            .query_keys
            .iter()
            .map(ClientDbSourceIndexQueryKey::as_str)
            .collect::<Vec<_>>(),
        ["gerbil-poo", "poo usage"]
    );
    assert!(raw_source_like_columns(&db_path).is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_reuses_generation_when_file_hashes_are_unchanged() {
    let root = temp_root("source-index-reuse");
    let db_path = root.join("client.sqlite3");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    let mut first_import = source_index(&root, "source-main-1", "src/lib.ss");
    first_import.file_hashes.push(ClientCacheFileHash {
        path: "@scope/dir/src".to_string(),
        sha256: "0".repeat(64),
        byte_len: 1,
        mtime_ms: 2,
    });
    let mut second_import = source_index(&root, "source-main-2", "src/lib.ss");
    second_import.file_hashes.push(ClientCacheFileHash {
        path: "@scope/dir/src".to_string(),
        sha256: "0".repeat(64),
        byte_len: 1,
        mtime_ms: 2,
    });
    second_import.file_hashes.reverse();

    let first = db
        .replace_source_index(&first_import)
        .expect("write first generation");
    let second = db
        .replace_source_index(&second_import)
        .expect("reuse first generation");
    let summary = db.summary().expect("db summary");

    assert_eq!(first, second);
    assert_eq!(second.generation_id.as_str(), "source-main-1");
    assert_eq!(summary.source_index_generation_count, 1);
    assert_eq!(summary.source_index_owner_count, 1);
    assert_eq!(summary.source_index_selector_count, 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_lookup_uses_latest_project_generation() {
    let root = temp_root("source-index-latest");
    let db_path = root.join("client.sqlite3");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.replace_source_index(&source_index(&root, "source-main-1", "src/old.ss"))
        .expect("write old generation");
    db.replace_source_index(&source_index(&root, "source-main-2", "src/new.ss"))
        .expect("write latest generation");
    let owners = db
        .lookup_source_index_owners(&lookup(&root, "gerbil-poo"))
        .expect("lookup source owners");
    let summary = db.summary().expect("db summary");

    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].owner_path.as_str(), "src/new.ss");
    assert_eq!(summary.source_index_generation_count, 2);
    assert_eq!(summary.source_index_owner_count, 2);
    assert_eq!(summary.source_index_selector_count, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_lookup_filters_language_scope() {
    let root = temp_root("source-index-language-scope");
    let db_path = root.join("client.sqlite3");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    let mut source_index = source_index(&root, "source-main-1", "src/lib.ss");
    source_index.file_hashes.push(ClientCacheFileHash {
        path: "src/lib.py".to_string(),
        sha256: "1".repeat(64),
        byte_len: 0,
        mtime_ms: 0,
    });
    source_index.owners.push(ClientDbSourceIndexOwner {
        owner_path: ClientDbSourceIndexPath::from("src/lib.py"),
        language_id: Some(LanguageId::from("python")),
        provider_id: Some(ProviderId::from("rust-sql")),
        source_kind: ClientDbSourceIndexSource::from("python-source"),
        line_count: Some(12),
        query_keys: vec![ClientDbSourceIndexQueryKey::from("gerbil-poo")],
    });
    source_index.selectors.push(ClientDbSourceIndexSelector {
        owner_path: ClientDbSourceIndexPath::from("src/lib.py"),
        selector_id: "src/lib.py:1:12".to_string(),
        symbol: Some("gerbil_poo".to_string()),
        kind: Some("function".to_string()),
        start_line: 1,
        end_line: 12,
        source: ClientDbSourceIndexSource::from("rust-sql"),
        query_keys: vec![ClientDbSourceIndexQueryKey::from("gerbil-poo")],
    });

    db.replace_source_index(&source_index)
        .expect("replace source index");
    let gerbil_owners = db
        .lookup_source_index_owners(&ClientDbSourceIndexLookup {
            project_root: root.clone(),
            language_id: Some(LanguageId::from("gerbil-scheme")),
            query: ClientDbSourceIndexQueryKey::from("gerbil-poo"),
            limit: 8,
        })
        .expect("lookup gerbil owners");
    let python_owners = db
        .lookup_source_index_owners(&ClientDbSourceIndexLookup {
            project_root: root.clone(),
            language_id: Some(LanguageId::from("python")),
            query: ClientDbSourceIndexQueryKey::from("gerbil-poo"),
            limit: 8,
        })
        .expect("lookup python owners");

    assert_eq!(gerbil_owners.len(), 1);
    assert_eq!(gerbil_owners[0].owner_path.as_str(), "src/lib.ss");
    assert_eq!(python_owners.len(), 1);
    assert_eq!(python_owners[0].owner_path.as_str(), "src/lib.py");
    let _ = std::fs::remove_dir_all(root);
}

fn source_index(
    root: &std::path::Path,
    generation_id: &str,
    owner_path: &str,
) -> ClientDbSourceIndexImport {
    ClientDbSourceIndexImport {
        generation_id: CacheGenerationId::from(generation_id),
        project_root: root.to_path_buf(),
        schema_id: "agent.semantic-protocols.semantic-source-index".into(),
        schema_version: "1".into(),
        file_hashes: vec![ClientCacheFileHash {
            path: owner_path.to_string(),
            sha256: "0".repeat(64),
            byte_len: 0,
            mtime_ms: 0,
        }],
        owners: vec![ClientDbSourceIndexOwner {
            owner_path: ClientDbSourceIndexPath::from(owner_path),
            language_id: Some(LanguageId::from("gerbil-scheme")),
            provider_id: Some(ProviderId::from("rust-sql")),
            source_kind: ClientDbSourceIndexSource::from("scheme-source"),
            line_count: Some(80),
            query_keys: vec![
                ClientDbSourceIndexQueryKey::from("gerbil-poo"),
                ClientDbSourceIndexQueryKey::from("poo usage"),
            ],
        }],
        selectors: vec![ClientDbSourceIndexSelector {
            owner_path: ClientDbSourceIndexPath::from(owner_path),
            selector_id: format!("{owner_path}:12:20"),
            symbol: Some("poo-read".to_string()),
            kind: Some("function".to_string()),
            start_line: 12,
            end_line: 20,
            source: ClientDbSourceIndexSource::from("rust-sql"),
            query_keys: vec![ClientDbSourceIndexQueryKey::from("gerbil-poo://usage")],
        }],
    }
}

fn lookup(root: &std::path::Path, query: &str) -> ClientDbSourceIndexLookup {
    ClientDbSourceIndexLookup {
        project_root: root.to_path_buf(),
        language_id: None,
        query: ClientDbSourceIndexQueryKey::from(query),
        limit: 8,
    }
}

fn raw_source_like_columns(db_path: &std::path::Path) -> Vec<String> {
    let conn = rusqlite::Connection::open(db_path).expect("open sqlite");
    let mut statement = conn
        .prepare(
            "SELECT m.name, p.name
             FROM sqlite_master m
             JOIN pragma_table_info(m.name) p
             WHERE m.type = 'table'
               AND m.name LIKE 'source_index_%'
               AND (
                    p.name LIKE '%source_text%'
                    OR p.name LIKE '%code%'
                    OR p.name LIKE '%snippet%'
                    OR p.name LIKE '%window%'
               )
             ORDER BY m.name, p.name",
        )
        .expect("prepare table info");
    statement
        .query_map([], |row| {
            Ok(format!(
                "{}.{}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?
            ))
        })
        .expect("query table info")
        .map(|row| row.expect("table info row"))
        .collect()
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-db-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}
