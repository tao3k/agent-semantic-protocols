#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Shared Tokio/Hyper HTTP JSON host used by ASP protocol bindings.

pub mod persistent;
mod server;

pub use server::HttpJsonRequest;
pub use server::HttpJsonResponse;
pub use server::post_json;
pub use server::run_http_json;
pub use server::serve_http_json;
pub use server::serve_http_json_h2;
