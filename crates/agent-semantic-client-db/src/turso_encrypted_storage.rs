// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Explicit opt-in Turso 0.7 local at-rest encryption profile.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Schema identifier for encrypted-file verification receipts.
pub const TURSO_ENCRYPTION_FILE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.client-db.turso-encryption-file-receipt.v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Supported Turso at-rest encryption cipher.
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
/// Validated hexadecimal key whose debug representation is always redacted.
pub struct TursoEncryptionKey(String);

impl TursoEncryptionKey {
    /// Parse a key whose length is valid for the selected cipher.
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
/// Path, cipher, and redacted key for an encrypted local profile.
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
/// File-level evidence that persisted bytes do not expose a plaintext probe.
pub struct TursoEncryptionFileReceipt {
    schema_id: TursoEncryptionFileReceiptSchemaId,
    cipher: TursoEncryptionCipher,
    database_bytes: u64,
    wal_bytes: u64,
    shm_bytes: u64,
    plaintext_probe_len: usize,
    plaintext_probe_present: bool,
}

struct TursoEncryptionFileReceiptInput {
    schema_id: TursoEncryptionFileReceiptSchemaId,
    cipher: TursoEncryptionCipher,
    database_bytes: u64,
    wal_bytes: u64,
    shm_bytes: u64,
    plaintext_probe_len: usize,
    plaintext_probe_present: bool,
}

impl TursoEncryptionFileReceipt {
    fn new(input: TursoEncryptionFileReceiptInput) -> Self {
        Self {
            schema_id: input.schema_id,
            cipher: input.cipher,
            database_bytes: input.database_bytes,
            wal_bytes: input.wal_bytes,
            shm_bytes: input.shm_bytes,
            plaintext_probe_len: input.plaintext_probe_len,
            plaintext_probe_present: input.plaintext_probe_present,
        }
    }

    /// Stable schema identity of this encryption receipt.
    #[must_use]
    pub fn schema_id(&self) -> &str {
        self.schema_id.as_str()
    }

    /// Cipher used by the encrypted database.
    #[must_use]
    pub const fn cipher(&self) -> &TursoEncryptionCipher {
        &self.cipher
    }

    /// Bytes occupied by the main database file.
    #[must_use]
    pub const fn database_bytes(&self) -> u64 {
        self.database_bytes
    }

    /// Bytes occupied by the WAL file.
    #[must_use]
    pub const fn wal_bytes(&self) -> u64 {
        self.wal_bytes
    }

    /// Bytes occupied by the shared-memory file.
    #[must_use]
    pub const fn shm_bytes(&self) -> u64 {
        self.shm_bytes
    }

    /// Length of the plaintext probe used by the verification pass.
    #[must_use]
    pub const fn plaintext_probe_len(&self) -> usize {
        self.plaintext_probe_len
    }

    /// Whether the plaintext probe was found in encrypted files.
    #[must_use]
    pub const fn plaintext_probe_present(&self) -> bool {
        self.plaintext_probe_present
    }
}

/// Open encrypted Turso database and its file-verification owner.
pub struct TursoEncryptedStorage {
    _database: turso::Database,
    connection: turso::Connection,
    path: PathBuf,
    cipher: TursoEncryptionCipher,
}

impl TursoEncryptedStorage {
    /// Open an encrypted local database from a validated profile.
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

    /// Open a connection to the encrypted database.
    pub fn connection(&self) -> turso::Connection {
        self.connection.clone()
    }

    /// Flush encrypted storage and inspect its persisted file surfaces.
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
        Ok(TursoEncryptionFileReceipt::new(
            TursoEncryptionFileReceiptInput {
                schema_id: TURSO_ENCRYPTION_FILE_RECEIPT_SCHEMA_ID.into(),
                cipher: self.cipher,
                database_bytes: database.len() as u64,
                wal_bytes: wal.len() as u64,
                shm_bytes: shm.len() as u64,
                plaintext_probe_len: plaintext_probe.len(),
                plaintext_probe_present,
            },
        ))
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
/// Typed schema identity carried by an encryption file receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TursoEncryptionFileReceiptSchemaId(String);

impl From<&str> for TursoEncryptionFileReceiptSchemaId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl TursoEncryptionFileReceiptSchemaId {
    /// Borrow the schema identifier.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
