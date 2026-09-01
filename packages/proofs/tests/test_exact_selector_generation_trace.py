from __future__ import annotations

import pytest

from asp_proofs.exact_selector_generation_trace import (
    MutationWitness,
    MutationWitnessSource,
    RefinementViolation,
    RuntimeState,
    Transition,
    TransitionKind,
    replay,
)


WORKSPACE = "workspace-23cc5ba784c605ae"
OWNER = "languages/asp-rust/src/exact_source_projection.rs"


def mutation(workspace_id: str = WORKSPACE) -> Transition:
    return Transition(
        kind=TransitionKind.OBSERVE_MUTATION,
        workspace_id=workspace_id,
        witness=MutationWitness(
            workspace_id=workspace_id,
            mutation_id="projection-packet-renamed",
            owner_path=OWNER,
            content_digest="a" * 64,
            source=MutationWitnessSource.RESIDENT_WATCHER,
        ),
    )


def initial() -> RuntimeState:
    return RuntimeState(
        workspace_id=WORKSPACE,
        active_generation_digest="blake3-256:" + "b" * 64,
    )


def test_projection_packet_counterexample_is_denied_before_reconciliation() -> None:
    with pytest.raises(
        RefinementViolation,
        match="unreconciled same-workspace mutation",
    ):
        replay(
            initial(),
            [
                mutation(),
                Transition(kind=TransitionKind.EXACT_READ, workspace_id=WORKSPACE),
            ],
        )


def test_complete_successor_publication_releases_exact_read() -> None:
    final = replay(
        initial(),
        [
            mutation(),
            Transition(
                kind=TransitionKind.PUBLISH_GENERATION,
                workspace_id=WORKSPACE,
                next_generation_digest="blake3-256:" + "c" * 64,
            ),
            Transition(kind=TransitionKind.EXACT_READ, workspace_id=WORKSPACE),
        ],
    )
    assert final.pending == ()
    assert final.active_generation_digest == "blake3-256:" + "c" * 64


def test_other_workspace_mutation_preserves_isolation() -> None:
    final = replay(
        initial(),
        [
            mutation("workspace-other"),
            Transition(kind=TransitionKind.EXACT_READ, workspace_id=WORKSPACE),
        ],
    )
    assert final == initial()
