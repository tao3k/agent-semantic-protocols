<!--
SPDX-FileCopyrightText: 2026 tao3k team and Contributors
SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only
-->

# Expected

ASP resolves the dependency package with Cargo metadata, emits `sourceRoot`, and returns an `external-api` selector or explicit API miss without requiring manual `.cargo/git/checkouts` discovery.
