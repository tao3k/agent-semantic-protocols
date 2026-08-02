"""Executable refinement oracle for exact-selector generation admission.

The Lean model is the formal authority. This module checks concrete Runtime
Server traces against the same transition boundary without reimplementing
search or provider behavior.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from enum import StrEnum


class TransitionKind(StrEnum):
    OBSERVE_MUTATION = "observe-mutation"
    PUBLISH_GENERATION = "publish-generation"
    EXACT_READ = "exact-read"


class MutationWitnessSource(StrEnum):
    CLIENT_HOOK = "client-hook"
    RESIDENT_WATCHER = "resident-watcher"


@dataclass(frozen=True)
class MutationWitness:
    workspace_id: str
    mutation_id: str
    owner_path: str
    content_digest: str
    source: MutationWitnessSource

    def validate(self) -> None:
        values = (
            self.workspace_id,
            self.mutation_id,
            self.owner_path,
            self.content_digest,
        )
        if any(not value.strip() for value in values):
            raise RefinementViolation("mutation witness contains empty authority")


@dataclass(frozen=True)
class RuntimeState:
    workspace_id: str
    active_generation_digest: str
    pending: tuple[MutationWitness, ...] = ()


@dataclass(frozen=True)
class Transition:
    kind: TransitionKind
    workspace_id: str
    witness: MutationWitness | None = None
    next_generation_digest: str | None = None


class RefinementViolation(ValueError):
    """A concrete trace cannot refine the formal transition system."""


def refine_transition(state: RuntimeState, transition: Transition) -> RuntimeState:
    if not transition.workspace_id.strip():
        raise RefinementViolation("transition omitted workspace identity")
    if transition.kind is TransitionKind.OBSERVE_MUTATION:
        witness = transition.witness
        if witness is None:
            raise RefinementViolation("mutation transition omitted its witness")
        witness.validate()
        if witness.workspace_id != transition.workspace_id:
            raise RefinementViolation("transition and witness workspace identities differ")
        if witness.workspace_id != state.workspace_id:
            return state
        return replace(state, pending=(*state.pending, witness))
    if transition.workspace_id != state.workspace_id:
        return state
    if transition.kind is TransitionKind.PUBLISH_GENERATION:
        digest = transition.next_generation_digest
        if digest is None or not digest.strip():
            raise RefinementViolation("publication omitted successor generation digest")
        if not state.pending:
            raise RefinementViolation("publication has no witnessed mutation to reconcile")
        return replace(state, active_generation_digest=digest, pending=())
    if transition.kind is TransitionKind.EXACT_READ:
        if state.pending:
            raise RefinementViolation(
                "exact read crossed an unreconciled same-workspace mutation"
            )
        return state
    raise RefinementViolation(f"unknown transition kind: {transition.kind}")


def replay(initial: RuntimeState, transitions: list[Transition]) -> RuntimeState:
    state = initial
    for transition in transitions:
        state = refine_transition(state, transition)
    return state
