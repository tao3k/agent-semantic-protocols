# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from pathlib import Path


LEGACY_RUNTIME_AUTHORITIES = (
    'runtime/bin',
    'runtime/resident',
    'runtime/profiles',
    'runtime/provider-artifacts',
    'runtime/providers',
    'runtime/artifact-identities',
    'runtime/provider-catalog.v1.json',
    'runtime/installed-provider-artifacts.json',
)


def test_production_rust_has_one_artifacts_owned_runtime_state_authority() -> None:
    root = Path(__file__).resolve().parents[2]
    violations: list[str] = []
    for path in sorted((root / 'crates').glob('*/src/**/*.rs')):
        # This module is the one-way migration/cleanup boundary and therefore
        # must be able to name paths that no production reader may use.
        if path.name == 'runtime_state_cleanup.rs':
            continue
        source = path.read_text(encoding='utf-8')
        for legacy in LEGACY_RUNTIME_AUTHORITIES:
            if legacy in source:
                violations.append(f'{path.relative_to(root)}: {legacy}')

    assert violations == [], (
        'legacy Runtime authorities remain in production source; all Runtime '
        'state paths must be issued by agent-semantic-artifacts:\n'
        + '\n'.join(violations)
    )
