// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
