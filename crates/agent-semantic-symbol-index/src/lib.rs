#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Language-neutral P0 Symbol Skeleton Index facade.

mod symbol_skeleton_index;

pub use symbol_skeleton_index::{
    SymbolSkeletonHitV1, SymbolSkeletonIndexV1, SymbolSkeletonOwnerV1, SymbolSkeletonRecordV1,
    symbol_skeleton_navigation_keys, symbol_skeleton_terms,
};
