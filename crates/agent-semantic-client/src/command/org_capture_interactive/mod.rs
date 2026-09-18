// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Contract-owned interactive windows for `asp org capture`.

mod choice;

pub(crate) use choice::AgentInteractiveChoice;
pub(super) use choice::choice_arg_value;
pub(super) use choice::strip_choice_args;
