// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Static limits and filename catalogs for DB Engine source indexing.

pub(super) use crate::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID as SOURCE_INDEX_PROVIDER_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_ID as SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION as SOURCE_INDEX_SCHEMA_VERSION,
};
pub(super) const SOURCE_INDEX_FILE_BYTES_LIMIT: u64 = 1_048_576;
