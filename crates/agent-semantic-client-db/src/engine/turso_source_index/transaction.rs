// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TursoSourceIndexWriteStats {
    pub(super) physical_generation_id: String,
    pub(super) changed_owner_count: u32,
    pub(super) removed_owner_count: u32,
    pub(super) posting_write_count: u32,
}
