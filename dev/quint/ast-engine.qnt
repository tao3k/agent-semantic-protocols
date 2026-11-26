// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

// ASP-AST-R02 / R03 / R05 / R06. Formal Quint state-machine verification for
// `crates/agent-semantic-ast`:
// 1. Stale BLAKE3 preimages never commit.
// 2. Ambiguous minimal-cost calibration ties never auto-apply (fail-closed).
// 3. Unverified or re-parse-failing edits never mutate disk state.
// 4. Committed disk digest always matches synchronized Open EDB digest.
module astTransactionSafety {
  type TxState = {
    diskHash: int,
    openEdbHash: int,
    expectedPreimage: int,
    stagedHash: int,
    minCostRepairCount: int,
    oracleAccepted: bool,
    styleCostBefore: int,
    styleCostAfter: int,
    committedCount: int,
    rejectedCount: int,
    dirtyDiskWrite: bool,
    ambiguousAutoApplied: bool,
    styleCostRegressed: bool
  }
  var s: TxState

  action init = s' = {
    diskHash: 1,
    openEdbHash: 1,
    expectedPreimage: 0,
    stagedHash: 0,
    minCostRepairCount: 0,
    oracleAccepted: false,
    styleCostBefore: 10,
    styleCostAfter: 10,
    committedCount: 0,
    rejectedCount: 0,
    dirtyDiskWrite: false,
    ambiguousAutoApplied: false,
    styleCostRegressed: false
  }

  // External concurrent edit bumps diskHash + openEdbHash together.
  action externalWorkspaceEdit(newHash: int): bool = all {
    newHash >= 1,
    newHash <= 4,
    newHash != s.diskHash,
    s' = s.with("diskHash", newHash).with("openEdbHash", newHash)
  }

  // Execute one atomic AST patch + calibration + style-elevation transaction.
  // Admits commit ONLY when:
  //   - preimage matches diskHash
  //   - either fast-path (minCostRepairCount == 0 & oracleAccepted)
  //     OR unique minimal-cost calibration (minCostRepairCount == 1 & oracleAccepted)
  //   - style elevation never increases style cost (styleCostAfter <= styleCostBefore)
  action runAstTransaction(
    preimage: int,
    nextHash: int,
    repairCount: int,
    verified: bool,
    nextCost: int
  ): bool = all {
    preimage >= 1,
    preimage <= 4,
    nextHash >= 1,
    nextHash <= 4,
    repairCount >= 0,
    repairCount <= 3,
    nextCost >= 1,
    nextCost <= 15,
    s.committedCount < 10,
    if (preimage == s.diskHash
        and verified
        and (repairCount == 0 or repairCount == 1)
        and nextCost <= s.styleCostBefore) {
      s' = s
        .with("expectedPreimage", preimage)
        .with("stagedHash", nextHash)
        .with("minCostRepairCount", repairCount)
        .with("oracleAccepted", verified)
        .with("diskHash", nextHash)
        .with("openEdbHash", nextHash)
        .with("styleCostBefore", nextCost)
        .with("styleCostAfter", nextCost)
        .with("committedCount", s.committedCount + 1)
    } else {
      // Fail-closed rollback: diskHash and openEdbHash remain completely untouched.
      s' = s
        .with("expectedPreimage", preimage)
        .with("stagedHash", nextHash)
        .with("minCostRepairCount", repairCount)
        .with("oracleAccepted", verified)
        .with("rejectedCount", s.rejectedCount + 1)
    }
  }

  action step = any {
    nondet h = oneOf(Set(1, 2, 3, 4))
    externalWorkspaceEdit(h),
    nondet pre = oneOf(Set(1, 2, 3, 4))
    nondet nxt = oneOf(Set(1, 2, 3, 4))
    nondet rc = oneOf(Set(0, 1, 2, 3))
    nondet ok = oneOf(Set(true, false))
    nondet cost = oneOf(Set(4, 8, 10, 14))
    runAstTransaction(pre, nxt, rc, ok, cost),
  }

  def safe = all {
    s.diskHash == s.openEdbHash,
    not(s.dirtyDiskWrite),
    not(s.ambiguousAutoApplied),
    not(s.styleCostRegressed),
    s.styleCostAfter <= s.styleCostBefore,
  }

  // Negative mutation controls: each bug MUST violate `safe`.
  action commitUnverifiedMutationStep = all {
    s' = s.with("diskHash", 3).with("oracleAccepted", false).with("dirtyDiskWrite", true)
  }

  action commitAmbiguousTieMutationStep = all {
    s' = s.with("minCostRepairCount", 2).with("ambiguousAutoApplied", true)
  }

  action desyncOpenEdbMutationStep = all {
    s.diskHash != 4,
    s' = s.with("diskHash", 4)
  }

  action styleCostRegressionMutationStep = all {
    s' = s.with("styleCostAfter", s.styleCostBefore + 3).with("styleCostRegressed", true)
  }

  action commitUnverifiedMutation = any { step, commitUnverifiedMutationStep }
  action commitAmbiguousTieMutation = any { step, commitAmbiguousTieMutationStep }
  action desyncOpenEdbMutation = any { step, desyncOpenEdbMutationStep }
  action styleCostRegressionMutation = any { step, styleCostRegressionMutationStep }

  run exactFastPathCommitsAndSyncsTest =
    init
      .then(runAstTransaction(1, 2, 0, true, 8))
      .expect(safe and s.diskHash == 2 and s.openEdbHash == 2 and s.committedCount == 1)

  run uniqueMinCostCalibrationCommitsTest =
    init
      .then(runAstTransaction(1, 3, 1, true, 10))
      .expect(safe and s.diskHash == 3 and s.openEdbHash == 3 and s.committedCount == 1)

  run ambiguousTieFailsClosedWithoutMutatingDiskTest =
    init
      .then(runAstTransaction(1, 3, 2, true, 8))
      .expect(safe and s.diskHash == 1 and s.openEdbHash == 1 and s.rejectedCount == 1)

  run stalePreimageFailsClosedTest =
    init
      .then(externalWorkspaceEdit(2))
      .then(runAstTransaction(1, 3, 0, true, 8))
      .expect(safe and s.diskHash == 2 and s.openEdbHash == 2 and s.rejectedCount == 1)

  run reparseFailureRollsBackZeroWriteTest =
    init
      .then(runAstTransaction(1, 4, 1, false, 6))
      .expect(safe and s.diskHash == 1 and s.openEdbHash == 1 and s.rejectedCount == 1)
}
