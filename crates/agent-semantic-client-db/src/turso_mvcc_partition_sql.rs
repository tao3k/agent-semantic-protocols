pub(crate) const CREATE_PARTITION_TABLES_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS asp_mvcc_partition_head (
    partition_key TEXT PRIMARY KEY,
    revision INTEGER NOT NULL CHECK (revision >= 0),
    head_digest TEXT NOT NULL,
    last_sequence INTEGER NOT NULL CHECK (last_sequence >= 0),
    projection BLOB NOT NULL,
    committed_at_ms INTEGER NOT NULL CHECK (committed_at_ms >= 0)
);
CREATE TABLE IF NOT EXISTS asp_mvcc_partition_record (
    partition_key TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    record_id TEXT NOT NULL,
    record_kind TEXT NOT NULL,
    payload BLOB NOT NULL,
    committed_at_ms INTEGER NOT NULL CHECK (committed_at_ms >= 0),
    PRIMARY KEY (partition_key, sequence)
);
"#;

pub(crate) const SELECT_HEAD_SQL: &str = "SELECT revision, head_digest, last_sequence, projection, committed_at_ms \
     FROM asp_mvcc_partition_head WHERE partition_key = ?1";

pub(crate) const INSERT_HEAD_SQL: &str = "INSERT INTO asp_mvcc_partition_head \
     (partition_key, revision, head_digest, last_sequence, projection, committed_at_ms) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

pub(crate) const UPDATE_HEAD_SQL: &str = "UPDATE asp_mvcc_partition_head \
     SET revision = ?2, head_digest = ?3, last_sequence = ?4, \
         projection = ?5, committed_at_ms = ?6 \
     WHERE partition_key = ?1 AND revision = ?7 \
       AND head_digest = ?8 AND last_sequence = ?9";

pub(crate) const SELECT_RECORD_ID_SQL: &str = "SELECT 1 FROM asp_mvcc_partition_record \
     WHERE partition_key = ?1 AND record_id = ?2 LIMIT 1";

pub(crate) const INSERT_RECORD_SQL: &str = "INSERT INTO asp_mvcc_partition_record \
     (partition_key, sequence, record_id, record_kind, payload, committed_at_ms) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

pub(crate) const SELECT_RECORDS_SQL: &str = "SELECT sequence, record_id, record_kind, payload, committed_at_ms \
     FROM asp_mvcc_partition_record WHERE partition_key = ?1 ORDER BY sequence";
