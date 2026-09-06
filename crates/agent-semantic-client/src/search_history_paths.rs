// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Path and locator helpers for search history artifact events.

use std::path::PathBuf;

pub(super) fn target_or_query<'a>(target: &'a str, query: &'a str) -> &'a str {
    if target.is_empty() { query } else { target }
}

pub(super) fn target_path(value: &str) -> Option<PathBuf> {
    let value = strip_locator(value);
    if value.is_empty() || value.contains(' ') {
        return None;
    }
    let path = PathBuf::from(&value);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if !value.contains('/') && !file_name.contains('.') {
        return None;
    }
    Some(path)
}

fn strip_locator(value: &str) -> String {
    let Some((head, tail)) = value.rsplit_once(':') else {
        return value.to_string();
    };
    if tail.chars().all(|ch| ch.is_ascii_digit()) || tail.contains('-') {
        head.to_string()
    } else {
        value.to_string()
    }
}
