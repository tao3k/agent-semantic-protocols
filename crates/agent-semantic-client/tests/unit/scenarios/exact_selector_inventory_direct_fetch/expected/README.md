<!--
SPDX-FileCopyrightText: 2026 tao3k team and Contributors
SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only
-->

Expected route:

- metadata route: main-agent direct inventory
- final evidence route: main-agent direct fetch via `--content` or `--code`
- subagent dispatch count: 0 unless fan-out or synthesis is required
- duplicate model read count: 0
- forbidden routes for this scenario: search-prime, search-pipe, raw-read
