\* SPDX-FileCopyrightText: 2026 tao3k team and Contributors
\*
\* SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

------------------------ MODULE SearchQueryEvidenceHandoff ------------------------
EXTENDS Naturals, TLC

CONSTANTS MaxEvidence, QueryGrammarText

ASSUME MaxEvidence = 30
ASSUME QueryGrammarText =
  "asp query playbook --language <producer|...> --selector <exact-selector> --projection <callable-skeleton|source>"

VARIABLES
  phase,
  evidenceCount,
  queryGrammar,
  sourceMaterialized,
  agentChoice

vars ==
  <<phase, evidenceCount, queryGrammar, sourceMaterialized, agentChoice>>

Init ==
  /\ phase = "searching"
  /\ evidenceCount = 0
  /\ queryGrammar = "none"
  /\ sourceMaterialized = FALSE
  /\ agentChoice = "undecided"

EmitCandidates(count) ==
  /\ phase = "searching"
  /\ count \in 1..MaxEvidence
  /\ phase' = "handoff"
  /\ evidenceCount' = count
  /\ queryGrammar' = QueryGrammarText
  /\ sourceMaterialized' = FALSE
  /\ agentChoice' = "undecided"

EmitEmpty ==
  /\ phase = "searching"
  /\ phase' = "done"
  /\ evidenceCount' = 0
  /\ queryGrammar' = "none"
  /\ sourceMaterialized' = FALSE
  /\ agentChoice' = "stop"

ChooseQuery ==
  /\ phase = "handoff"
  /\ phase' = "queried"
  /\ sourceMaterialized' = TRUE
  /\ agentChoice' = "query"
  /\ UNCHANGED <<evidenceCount, queryGrammar>>

ChooseRefine ==
  /\ phase = "handoff"
  /\ phase' = "searching"
  /\ evidenceCount' = 0
  /\ queryGrammar' = "none"
  /\ sourceMaterialized' = FALSE
  /\ agentChoice' = "refine"

ChooseStop ==
  /\ phase = "handoff"
  /\ phase' = "done"
  /\ sourceMaterialized' = FALSE
  /\ agentChoice' = "stop"
  /\ UNCHANGED <<evidenceCount, queryGrammar>>

Next ==
  \/ \E count \in 1..MaxEvidence : EmitCandidates(count)
  \/ EmitEmpty
  \/ ChooseQuery
  \/ ChooseRefine
  \/ ChooseStop

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in {"searching", "handoff", "queried", "done"}
  /\ evidenceCount \in 0..MaxEvidence
  /\ queryGrammar \in {"none", QueryGrammarText}
  /\ sourceMaterialized \in BOOLEAN
  /\ agentChoice \in {"undecided", "query", "refine", "stop"}

TopKBound == evidenceCount <= MaxEvidence

QueryGrammarExactlyWhenEvidenceExists ==
  (queryGrammar = QueryGrammarText) <=> (evidenceCount > 0)

SearchNeverMaterializesSource ==
  phase \in {"searching", "handoff", "done"} => sourceMaterialized = FALSE

SourceOnlyAfterAgentQuery ==
  sourceMaterialized => (phase = "queried" /\ agentChoice = "query")

ProviderDoesNotChooseContinuation ==
  phase = "handoff" => agentChoice = "undecided"

HandoffKeepsAgentChoicesOpen ==
  phase = "handoff" =>
    (ENABLED ChooseQuery /\ ENABLED ChooseRefine /\ ENABLED ChooseStop)

=============================================================================
