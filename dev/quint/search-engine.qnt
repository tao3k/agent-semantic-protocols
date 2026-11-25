// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

// ASP-SRCH-R03 / MAR-SRCH-R04. Content-addressed artifact CAS publication,
// monotonic revision advancement, and stale-writer CAS conflict fencing.
module searchGenerationPublication {
  type State = {
    blobs: Set[int],
    activeHash: int,
    activeGen: int,
    revision: int,
    prevRevision: int,
    casConflicts: int,
    danglingRead: bool,
    corruptedSwap: bool
  }
  var s: State

  action init = s' = {
    blobs: Set(),
    activeHash: 0,
    activeGen: 0,
    revision: 0,
    prevRevision: 0,
    casConflicts: 0,
    danglingRead: false,
    corruptedSwap: false
  }

  action putArtifact(hash: int): bool = all {
    hash >= 1,
    hash <= 4,
    s' = s.with("blobs", s.blobs.union(Set(hash)))
  }

  action publishCas(expectedHash: int, newHash: int, newGen: int): bool = all {
    newHash.in(s.blobs),
    newGen >= 1,
    newGen <= 4,
    s.revision < 10,
    if (expectedHash == s.activeHash) {
      s' = s
        .with("activeHash", newHash)
        .with("activeGen", newGen)
        .with("prevRevision", s.revision)
        .with("revision", s.revision + 1)
    } else {
      s' = s.with("casConflicts", s.casConflicts + 1)
    }
  }

  action loadActive = all {
    s.activeHash != 0,
    s' = s.with("danglingRead", not(s.activeHash.in(s.blobs)))
  }

  action step = any {
    nondet h = oneOf(Set(1, 2, 3, 4))
    putArtifact(h),
    nondet exp = oneOf(Set(0, 1, 2, 3, 4))
    nondet nh = oneOf(Set(1, 2, 3, 4))
    nondet ng = oneOf(Set(1, 2, 3, 4))
    publishCas(exp, nh, ng),
    loadActive,
  }

  def safe = all {
    s.activeHash != 0 implies (s.activeHash.in(s.blobs) and s.activeGen > 0 and s.revision > 0),
    s.activeHash == 0 implies (s.activeGen == 0 and s.revision == 0),
    s.revision >= s.prevRevision,
    not(s.danglingRead),
    not(s.corruptedSwap),
  }

  // Negative controls: each mutation must violate `safe`.
  action publishWithoutBlob = all {
    not(4.in(s.blobs)),
    s' = s.with("activeHash", 4).with("activeGen", 1).with("revision", s.revision + 1)
  }

  action overwriteIgnoringExpectedHash = all {
    s.activeHash != 0,
    2.in(s.blobs),
    s' = s.with("activeHash", 2).with("corruptedSwap", true)
  }

  action regressRevision = all {
    s.revision > 0,
    s' = s.with("prevRevision", s.revision).with("revision", s.revision - 1)
  }

  action publishWithoutBlobMutation = any { step, publishWithoutBlob }
  action bypassCasMutation = any { step, overwriteIgnoringExpectedHash }
  action nonMonotonicRevisionMutation = any { step, regressRevision }

  run initialPublishAndReadTest =
    init
      .then(putArtifact(1))
      .then(publishCas(0, 1, 1))
      .then(loadActive)
      .expect(safe and s.activeHash == 1 and s.revision == 1 and not(s.danglingRead))

  run staleCasConflictPreservesActivePointerTest =
    init
      .then(putArtifact(1))
      .then(publishCas(0, 1, 1))
      .then(putArtifact(2))
      .then(publishCas(0, 2, 2))
      .expect(safe and s.activeHash == 1 and s.revision == 1 and s.casConflicts == 1)

  run sequentialCasAdvancesRevisionTest =
    init
      .then(putArtifact(1))
      .then(publishCas(0, 1, 1))
      .then(putArtifact(2))
      .then(publishCas(1, 2, 2))
      .then(loadActive)
      .expect(safe and s.activeHash == 2 and s.activeGen == 2 and s.revision == 2)
}

// ASP-SRCH-R01 / MAR-SRCH-R02. Native Gerbil `RootedSchemeObject` ABI
// admission gate: rejects pseudo S-expression text and scope traversal fail-closed.
module searchPlanAbiAdmission {
  type State = {
    validRootedVector: bool,
    scopeTraversal: bool,
    pseudoSexpr: bool,
    admitted: bool,
    admittedGen: int,
    activeGen: int,
    executed: bool,
    resultGen: int,
    forbiddenExecution: bool
  }
  var s: State

  action init = s' = {
    validRootedVector: false,
    scopeTraversal: false,
    pseudoSexpr: false,
    admitted: false,
    admittedGen: 0,
    activeGen: 1,
    executed: false,
    resultGen: 0,
    forbiddenExecution: false
  }

  action submitValidPlan = s' = s
    .with("validRootedVector", true)
    .with("scopeTraversal", false)
    .with("pseudoSexpr", false)
    .with("admitted", false)
    .with("executed", false)

  action submitScopeTraversalPlan = s' = s
    .with("validRootedVector", true)
    .with("scopeTraversal", true)
    .with("pseudoSexpr", false)
    .with("admitted", false)
    .with("executed", false)

  action submitPseudoSexprPlan = s' = s
    .with("validRootedVector", false)
    .with("scopeTraversal", false)
    .with("pseudoSexpr", true)
    .with("admitted", false)
    .with("executed", false)

  action admitPlan = all {
    s.validRootedVector,
    not(s.scopeTraversal),
    not(s.pseudoSexpr),
    s' = s.with("admitted", true).with("admittedGen", s.activeGen).with("executed", false)
  }

  action advanceActiveGen = all {
    s.activeGen < 4,
    s' = s.with("activeGen", s.activeGen + 1).with("executed", false)
  }

  action executeAdmittedPlan = all {
    s.admitted,
    s.admittedGen == s.activeGen,
    s' = s.with("executed", true).with("resultGen", s.admittedGen)
  }

  action step = any {
    submitValidPlan,
    submitScopeTraversalPlan,
    submitPseudoSexprPlan,
    admitPlan,
    advanceActiveGen,
    executeAdmittedPlan,
  }

  def safe = all {
    s.admitted implies (s.validRootedVector and not(s.scopeTraversal) and not(s.pseudoSexpr)),
    s.executed implies (s.admitted and s.resultGen == s.admittedGen and s.resultGen > 0),
    not(s.forbiddenExecution),
  }

  // Negative controls.
  action executeUnadmittedPlan = all {
    not(s.admitted),
    s' = s.with("executed", true).with("forbiddenExecution", true)
  }

  action executeStaleGeneration = all {
    s.admitted,
    s.admittedGen != s.activeGen,
    s' = s.with("executed", true).with("resultGen", s.activeGen)
  }

  action bypassAdmissionMutation = any { step, executeUnadmittedPlan }
  action staleGenerationMutation = any { step, executeStaleGeneration }

  run validRootedPlanAdmitsAndExecutesTest =
    init
      .then(submitValidPlan)
      .then(admitPlan)
      .then(executeAdmittedPlan)
      .expect(safe and s.executed and s.resultGen == 1)

  run scopeTraversalFailsClosedTest =
    init
      .then(submitScopeTraversalPlan)
      .expect(safe and not(s.admitted) and not(s.executed))

  run pseudoSexprRejectedFailClosedTest =
    init
      .then(submitPseudoSexprPlan)
      .expect(safe and not(s.admitted) and not(s.executed))
}

// ASP-SRCH-R03 / MAR-SRCH-R04. Idempotent topology edge persistence and
// strict `(project_id, generation_digest)` query isolation.
module searchTopologyEdgeCustody {
  type Edge = (int, int, int)
  type State = {
    edges: Set[Edge],
    insertedCount: int,
    duplicateCount: int,
    lastQueryProject: int,
    lastQueryGen: int,
    lastQueryResult: Set[Edge]
  }
  var s: State

  action init = s' = {
    edges: Set(),
    insertedCount: 0,
    duplicateCount: 0,
    lastQueryProject: 1,
    lastQueryGen: 1,
    lastQueryResult: Set()
  }

  action appendEdge(proj: int, gen: int, eid: int): bool = all {
    proj >= 1, proj <= 2,
    gen >= 1, gen <= 2,
    eid >= 1, eid <= 3,
    val e: Edge = (proj, gen, eid)
    if (e.in(s.edges)) {
      s' = s.with("duplicateCount", s.duplicateCount + 1)
    } else {
      s' = s
        .with("edges", s.edges.union(Set(e)))
        .with("insertedCount", s.insertedCount + 1)
    }
  }

  action queryEdges(proj: int, gen: int): bool = all {
    proj >= 1, proj <= 2,
    gen >= 1, gen <= 2,
    val filtered = s.edges.filter(e => e._1 == proj and e._2 == gen)
    s' = s
      .with("lastQueryProject", proj)
      .with("lastQueryGen", gen)
      .with("lastQueryResult", filtered)
  }

  action step = any {
    nondet p = oneOf(Set(1, 2))
    nondet g = oneOf(Set(1, 2))
    nondet id = oneOf(Set(1, 2, 3))
    appendEdge(p, g, id),
    nondet p = oneOf(Set(1, 2))
    nondet g = oneOf(Set(1, 2))
    queryEdges(p, g),
  }

  def safe = all {
    s.insertedCount == s.edges.size(),
    s.lastQueryResult.subseteq(s.edges),
    s.lastQueryResult.forall(e => e._1 == s.lastQueryProject and e._2 == s.lastQueryGen),
  }

  // Negative controls.
  action leakCrossGenerationEdges = all {
    s.edges.size() > 0,
    s' = s.with("lastQueryProject", 1).with("lastQueryGen", 1).with("lastQueryResult", s.edges)
  }

  action doubleCountDuplicateEdge = all {
    val e: Edge = (1, 1, 1)
    e.in(s.edges),
    s' = s.with("insertedCount", s.insertedCount + 1)
  }

  action crossGenerationLeakMutation = any { step, leakCrossGenerationEdges }
  action duplicateCountMutation = any { step, doubleCountDuplicateEdge }

  run duplicateAppendIsIdempotentTest =
    init
      .then(appendEdge(1, 1, 1))
      .then(appendEdge(1, 1, 1))
      .expect(safe and s.insertedCount == 1 and s.duplicateCount == 1)

  run generationIsolationExcludesOtherGenerationsTest =
    init
      .then(appendEdge(1, 1, 1))
      .then(appendEdge(1, 2, 2))
      .then(queryEdges(1, 1))
      .expect(safe and s.lastQueryResult == Set((1, 1, 1)))
}
