<!--
SPDX-FileCopyrightText: 2026 tao3k team and Contributors
SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only
-->

# Expected

The command emits compact query-wrapper output with:

- `source=source-index`
- `sourceTrace=sourceIndex:used`
- `finder:skipped`
- `packages=src/lib.rs`
- no provider marker file

The scenario gate parses `collectMs` from stdout and requires it to stay within
`benchmark.toml` while provider and native finder process counts remain zero.
