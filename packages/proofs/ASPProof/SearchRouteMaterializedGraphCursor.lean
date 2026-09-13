-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouterInteractiveGraphState

namespace ASPProof.SearchRouteMaterializedGraphCursor

open ASPProof.SearchRouterInteractiveGraphState

def jumpAdmitted
    (state : GraphState)
    (targetNode nextStateDigest : NodeId) : Bool :=
  targetNode != state.currentNode &&
    state.nodeIds.contains targetNode &&
    !nextStateDigest.isEmpty &&
    nextStateDigest != state.stateDigest

def addPriorCurrentToVisited (state : GraphState) : List NodeId :=
  state.currentNode :: state.visited.erase state.currentNode

def jumpState
    (state : GraphState)
    (targetNode nextStateDigest : NodeId) : GraphState :=
  { state with
    stateDigest := nextStateDigest
    currentNode := targetNode
    visited := addPriorCurrentToVisited state }

theorem jump_preserves_session
    (state : GraphState)
    (targetNode nextStateDigest : NodeId) :
    (jumpState state targetNode nextStateDigest).sessionId = state.sessionId := by
  rfl

theorem jump_preserves_generation
    (state : GraphState)
    (targetNode nextStateDigest : NodeId) :
    (jumpState state targetNode nextStateDigest).generation = state.generation := by
  rfl

theorem jump_preserves_remaining_budget
    (state : GraphState)
    (targetNode nextStateDigest : NodeId) :
    (jumpState state targetNode nextStateDigest).remainingBudget =
      state.remainingBudget := by
  rfl

theorem jump_preserves_route_budget
    (state : GraphState)
    (targetNode nextStateDigest : NodeId) :
    (jumpState state targetNode nextStateDigest).routeBudget = state.routeBudget := by
  rfl

theorem jump_preserves_route_measure
    (state : GraphState)
    (targetNode nextStateDigest : NodeId) :
    (jumpState state targetNode nextStateDigest).routeMeasure =
      state.routeMeasure := by
  rfl

theorem jump_preserves_obligations
    (state : GraphState)
    (targetNode nextStateDigest : NodeId) :
    (jumpState state targetNode nextStateDigest).obligations = state.obligations := by
  rfl

theorem jump_preserves_frontier
    (state : GraphState)
    (targetNode nextStateDigest : NodeId) :
    (jumpState state targetNode nextStateDigest).frontier = state.frontier := by
  rfl

theorem jump_places_prior_current_at_visited_head
    (state : GraphState)
    (targetNode nextStateDigest : NodeId) :
    (jumpState state targetNode nextStateDigest).visited.head? =
      some state.currentNode := by
  rfl

theorem materialized_frontier_jump_is_admitted :
    jumpAdmitted ownershipState "symbol:ModelConfig" "state-7-jump" = true := by
  decide

def historyState : GraphState :=
  { ownershipState with
    generation := 8
    stateDigest := "state-8"
    currentNode := "symbol:AgentRegistry"
    nodeIds :=
      ["goal:model-owner", "symbol:ModelConfig", "symbol:AgentRegistry"]
    visited := ["goal:model-owner", "symbol:ModelConfig"] }

theorem materialized_history_jump_is_admitted :
    jumpAdmitted historyState "symbol:ModelConfig" "state-8-history-jump" = true := by
  decide

theorem unknown_node_jump_is_rejected :
    jumpAdmitted ownershipState "symbol:Unknown" "state-unknown" = false := by
  decide

theorem current_node_replay_is_rejected :
    jumpAdmitted ownershipState "goal:model-owner" "state-replay" = false := by
  decide

structure CursorArtifactBinding where
  loopId : String
  schemaId : String
  deriving DecidableEq

def cursorArtifactAdmitted
    (runtimeLoopId : String)
    (binding : CursorArtifactBinding) : Bool :=
  binding.loopId == runtimeLoopId &&
    binding.schemaId == "agent.semantic-protocols.search-graph-cursor"

def ownershipCursorArtifact : CursorArtifactBinding :=
  { loopId := "loop:search-1"
    schemaId := "agent.semantic-protocols.search-graph-cursor" }

theorem same_loop_cursor_artifact_is_admitted :
    cursorArtifactAdmitted "loop:search-1" ownershipCursorArtifact = true := by
  decide

theorem cross_loop_cursor_artifact_is_rejected :
    cursorArtifactAdmitted "loop:search-other" ownershipCursorArtifact = false := by
  decide

theorem wrong_schema_cursor_artifact_is_rejected :
    cursorArtifactAdmitted "loop:search-1"
      { ownershipCursorArtifact with
        schemaId := "agent.semantic-protocols.search-choice-panel" } = false := by
  decide

end ASPProof.SearchRouteMaterializedGraphCursor
