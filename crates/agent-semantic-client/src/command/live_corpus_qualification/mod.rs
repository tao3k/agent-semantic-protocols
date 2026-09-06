// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Live Corpus qualification command boundary.

pub(super) mod client_protocol;
mod contract;
mod runner;

pub(super) use runner::run;
pub(super) use runner::validate_args;
