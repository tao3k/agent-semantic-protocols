//! Explicit opt-in Turso 0.7 local at-rest encryption profile.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const TURSO_ENCRYPTION_FILE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.client-db.turso-encryption-file-receipt.v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TursoEncryptionCipher {
    Aegis256,
    Aes256Gcm,
    Aes128Gcm,
}

impl TursoEncryptionCipher {
    fn as_turso_cipher(self) -> &'static str {
        match self {
            Self::Aegis256 => "aegis256",
            Self::Aes256Gcm => "aes256gcm",
            Self::Aes128Gcm => "aes128gcm",
        }
    }

    fn key_bytes(self) -> usize {
        match self {
            Self::Aegis256 | Self::Aes256Gcm => 32,
            Self::Aes128Gcm => 16,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TursoEncryptionKey(String);

impl TursoEncryptionKey {
    pub fn from_hex(
        cipher: TursoEncryptionCipher,
        hex_key: impl Into<String>,
    ) -> Result<Self, String> {
        let hex_key = hex_key.into();
        let expected_len = cipher.key_bytes() * 2;
        if hex_key.len() != expected_len || !hex_key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "{} requires exactly {expected_len} hexadecimal key characters",
                cipher.as_turso_cipher()
            ));
        }
        Ok(Self(hex_key))
    }

    fn expose_for_builder(&self) -> String {
        self.0.clone()
    }
}

impl fmt::Debug for TursoEncryptionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TursoEncryptionKey([REDACTED])")
    }
}

#[derive(Clone)]
pub struct TursoEncryptedProfileConfig {
    pub path: PathBuf,
    pub cipher: TursoEncryptionCipher,
    pub key: TursoEncryptionKey,
}

impl fmt::Debug for TursoEncryptedProfileConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TursoEncryptedProfileConfig")
            .field("path", &self.path)
            .field("cipher", &self.cipher)
            .field("key", &self.key)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoEncryptionFileReceipt {
    schema_id: String,
    cipher: TursoEncryptionCipher,
    database_bytes: u64,
    wal_bytes: u64,
    shm_bytes: u64,
    plaintext_probe_len: usize,
    plaintext_probe_present: bool,
}

pub struct TursoEncryptedStorage {
    _database: turso::Database,
    connection: turso::Connection,
    path: PathBuf,
    cipher: TursoEncryptionCipher,
}

impl TursoEncryptedStorage {
    pub async fn open(config: TursoEncryptedProfileConfig) -> Result<Self, String> {
        let path = config.path.to_string_lossy();
        let database = turso::Builder::new_local(path.as_ref())
            .experimental_encryption(true)
            .with_encryption(turso::EncryptionOpts {
                cipher: config.cipher.as_turso_cipher().to_owned(),
                hexkey: config.key.expose_for_builder(),
            })
            .build()
            .await
            .map_err(|error| format!("failed to open encrypted Turso database: {error}"))?;
        let connection = database
            .connect()
            .map_err(|error| format!("failed to connect encrypted Turso database: {error}"))?;
        Ok(Self {
            _database: database,
            connection,
            path: config.path,
            cipher: config.cipher,
        })
    }

    pub fn connection(&self) -> turso::Connection {
        self.connection.clone()
    }

    pub async fn flush_and_measure(
        &self,
        plaintext_probe: &[u8],
    ) -> Result<TursoEncryptionFileReceipt, String> {
        self.connection
            .cacheflush()
            .map_err(|error| format!("failed to flush encrypted Turso database: {error}"))?;
        let database = read_if_present(&self.path)?;
        let wal_path = PathBuf::from(format!("{}-wal", self.path.to_string_lossy()));
        let shm_path = PathBuf::from(format!("{}-shm", self.path.to_string_lossy()));
        let wal = read_if_present(&wal_path)?;
        let shm = read_if_present(&shm_path)?;
        let plaintext_probe_present = !plaintext_probe.is_empty()
            && [database.as_slice(), wal.as_slice(), shm.as_slice()]
                .into_iter()
                .any(|bytes| contains_subslice(bytes, plaintext_probe));
        Ok(TursoEncryptionFileReceipt {
            schema_id: TURSO_ENCRYPTION_FILE_RECEIPT_SCHEMA_ID.to_owned(),
            cipher: self.cipher,
            database_bytes: database.len() as u64,
            wal_bytes: wal.len() as u64,
            shm_bytes: shm.len() as u64,
            plaintext_probe_len: plaintext_probe.len(),
            plaintext_probe_present,
        })
    }
}

fn read_if_present(path: &Path) -> Result<Vec<u8>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!(
            "failed to read encrypted database artifact: {error}"
        )),
    }
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
