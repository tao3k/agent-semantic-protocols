// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Thin aggregate for the isolated Live Corpus qualification process.

#[path = "live_corpus/runner.rs"]
mod runner;

pub use runner::main;
