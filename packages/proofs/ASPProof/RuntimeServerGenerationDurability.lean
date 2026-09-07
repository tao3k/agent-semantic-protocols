-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.RuntimeServerGenerationDurability

inductive PublicationState where
  | residentReady
  | durableReady
  | failed
  deriving DecidableEq, Repr

inductive ScopeState where
  | absent
  | prepared
  deriving DecidableEq, Repr

structure Publication where
  workspace : String
  epoch : Nat
  state : PublicationState

def queryAdmitted (publication : Publication) : Bool :=
  publication.state == .residentReady || publication.state == .durableReady

def exactQueryAdmitted (scope : ScopeState) (publication : Publication) : Bool :=
  scope == .prepared && queryAdmitted publication

def restartAdmitted (publication : Publication) : Bool :=
  publication.state == .durableReady

def mayPublishNext (publication : Publication) : Bool :=
  publication.state == .durableReady || publication.state == .failed

theorem resident_query_does_not_require_durability
    (publication : Publication)
    (ready : publication.state = .residentReady) :
    queryAdmitted publication = true := by
  simp [queryAdmitted, ready]

theorem absent_scope_cannot_enter_exact_query
    (publication : Publication) :
    exactQueryAdmitted .absent publication = false := by
  simp [exactQueryAdmitted]

theorem prepared_resident_generation_enters_exact_query
    (publication : Publication)
    (ready : publication.state = .residentReady) :
    exactQueryAdmitted .prepared publication = true := by
  simp [exactQueryAdmitted, queryAdmitted, ready]

theorem resident_generation_is_not_restart_authority
    (publication : Publication)
    (ready : publication.state = .residentReady) :
    restartAdmitted publication = false := by
  simp [restartAdmitted, ready]

theorem next_generation_cannot_overtake_resident_durability
    (publication : Publication)
    (ready : publication.state = .residentReady) :
    mayPublishNext publication = false := by
  simp [mayPublishNext, ready]

theorem durable_generation_is_query_and_restart_authority
    (publication : Publication)
    (durable : publication.state = .durableReady) :
    queryAdmitted publication = true /\ restartAdmitted publication = true := by
  simp [queryAdmitted, restartAdmitted, durable]

end ASPProof.RuntimeServerGenerationDurability
