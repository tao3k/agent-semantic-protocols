// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Thin `asp cache clean` command adapter.

use std::path::Path;

use super::clean_args::parse_cache_clean_args;

pub(crate) async fn run_cache_clean(
    project_root: &Path,
    forwarded_args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    let Some(args) = parse_cache_clean_args(forwarded_args)? else {
        return Ok(());
    };
    crate::cache_cleanup_service::apply_cache_cleanup(project_root, &args, receipt_json).await
}
