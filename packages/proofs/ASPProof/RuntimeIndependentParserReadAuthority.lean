-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.RuntimeIndependentParserReadAuthority

inductive RuntimeState
  | healthy
  | stopped
  | unavailable
  deriving DecidableEq

inductive GenerationState
  | missing
  | valid
  | stale
  | corrupt
  deriving DecidableEq

inductive ReadRoute
  | residentMemory
  | durableMmap
  | none
  deriving DecidableEq

inductive ReadState
  | ready
  | generationRequired
  | generationStale
  | generationCorrupt
  deriving DecidableEq

structure ReadAuthority where
  generation : GenerationState
  memoryLoaded : Bool

def acquire (_runtime : RuntimeState) (authority : ReadAuthority) : ReadRoute × ReadState :=
  match authority.generation with
  | .valid =>
      if authority.memoryLoaded then
        (.residentMemory, .ready)
      else
        (.durableMmap, .ready)
  | .missing => (.none, .generationRequired)
  | .stale => (.none, .generationStale)
  | .corrupt => (.none, .generationCorrupt)

def mayMutate : RuntimeState → Bool
  | .healthy => true
  | .stopped => false
  | .unavailable => false

theorem read_is_independent_of_runtime
    (authority : ReadAuthority)
    (left right : RuntimeState) :
    acquire left authority = acquire right authority := by
  cases authority.generation <;> cases authority.memoryLoaded <;> rfl

theorem valid_generation_is_readable_when_runtime_stopped
    (loaded : Bool) :
    (acquire .stopped { generation := .valid, memoryLoaded := loaded }).2 = .ready := by
  cases loaded <;> rfl

theorem valid_generation_never_uses_no_route
    (loaded : Bool) :
    (acquire .unavailable { generation := .valid, memoryLoaded := loaded }).1 ≠ .none := by
  cases loaded <;> decide

theorem invalid_generation_never_acquires_a_read_route
    (generation : GenerationState)
    (notValid : generation ≠ .valid)
    (loaded : Bool) :
    (acquire .healthy { generation := generation, memoryLoaded := loaded }).1 = .none := by
  cases generation <;> simp_all [acquire]

theorem stopped_runtime_cannot_mutate : mayMutate .stopped = false := by
  rfl

theorem unavailable_runtime_cannot_mutate : mayMutate .unavailable = false := by
  rfl

theorem runtime_health_changes_mutation_not_read
    (authority : ReadAuthority) :
    mayMutate .healthy ≠ mayMutate .stopped ∧
      acquire .healthy authority = acquire .stopped authority := by
  constructor
  · decide
  · exact read_is_independent_of_runtime authority .healthy .stopped

end ASPProof.RuntimeIndependentParserReadAuthority
