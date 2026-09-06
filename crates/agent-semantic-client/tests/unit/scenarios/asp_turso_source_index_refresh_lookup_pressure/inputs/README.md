<!--
SPDX-FileCopyrightText: 2026 tao3k team and Contributors
SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only
-->

# Inputs

The Rust libtest fixture creates a temporary Turso client DB and imports a
single Rust file named `src/source_index_pressure.rs`. A writer refreshes that
source-index generation several times while reader threads look up
`source_index_pressure_fixture`.
