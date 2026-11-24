#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Unified semantic identity, source-extension registry, and runtime configuration for ASP.

pub mod project_config;
pub mod runtime_dev;
mod semantic_identity;
pub mod source_extension;

pub use project_config::AspProjectConfigFile;
pub use project_config::AspProjectDiscoveryConfig;
pub use project_config::load_asp_project_config_file;
pub use semantic_identity::LanguageId;
pub use semantic_identity::ProviderId;
