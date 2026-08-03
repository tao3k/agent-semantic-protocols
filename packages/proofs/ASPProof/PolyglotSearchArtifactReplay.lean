import ASPProof.Generated.PolyglotSearchTraceFixture
import ASPProof.PolyglotSearchSemanticRefinement

namespace ASPProof.PolyglotSearchArtifactReplay

open ASPProof.PolyglotSearchSemanticRefinement
namespace Fixture
export ASPProof.Generated.PolyglotSearchTraceFixture
  (schemaId schemaVersion generatorId generatorVersion sourceSnapshotDigest
    gqlProfile logicProfile
    providerArtifactDigest runtimeArtifactDigest gqlSourceDigest gqlASTDigest
    gqlBindingDigest logicSourceDigest logicASTDigest logicBindingDigest
    graphDigestBefore graphDigestAfter registryDigest executionBindingDigest
    semanticTraceDigest payloadDigest multiplicity ordering gqlRows logicRows)
end Fixture

def projectedGQLRows : List (String × String) :=
  expectedGQLRows.map fun row => (toString row.functionId, toString row.candidateId)

def projectedLogicRows : List (String × String × Nat) :=
  expectedLogicRows.map fun row =>
    (toString row.functionId, toString row.candidateId, row.obligationId)

structure ArtifactReplayCheck where
  schemaId : String
  schemaVersion : String
  generatorId : String
  generatorVersion : String
  gqlProfile : String
  logicProfile : String
  graphDigestBefore : String
  graphDigestAfter : String
  payloadDigest : String
  gqlRows : List (String × String)
  logicRows : List (String × String × Nat)
  deriving DecidableEq

def committedCheck : ArtifactReplayCheck where
  schemaId := Fixture.schemaId
  schemaVersion := Fixture.schemaVersion
  generatorId := Fixture.generatorId
  generatorVersion := Fixture.generatorVersion
  gqlProfile := Fixture.gqlProfile
  logicProfile := Fixture.logicProfile
  graphDigestBefore := Fixture.graphDigestBefore
  graphDigestAfter := Fixture.graphDigestAfter
  payloadDigest := Fixture.payloadDigest
  gqlRows := Fixture.gqlRows
  logicRows := Fixture.logicRows

def replayAdmitted (check : ArtifactReplayCheck) : Bool :=
  check.schemaId == "agent.semantic-protocols.polyglot-search-trace.v1" &&
  check.schemaVersion == "1" &&
  check.generatorId == "asp-proofs-polyglot-search-trace" &&
  check.generatorVersion == "1" &&
  check.gqlProfile == "asp-gql-core:0.1-one-hop" &&
  check.logicProfile == "asp-logic-query-core:0.1-positive-conjunction" &&
  check.graphDigestBefore == check.graphDigestAfter &&
  check.payloadDigest ==
    "sha256:ce1c58d026a0740fedf87c08f4b6d8c67ed986ab29d9a86b87f00e2a8860b2ac" &&
  check.gqlRows == projectedGQLRows &&
  check.logicRows == projectedLogicRows

def tamperedPayloadCheck : ArtifactReplayCheck :=
  { committedCheck with payloadDigest :=
      "sha256:0000000000000000000000000000000000000000000000000000000000000000" }

theorem fixture_graph_is_read_only :
    Fixture.graphDigestBefore = Fixture.graphDigestAfter := by
  decide

theorem fixture_gql_rows_replay_reference_semantics :
    Fixture.gqlRows = projectedGQLRows := by
  decide

theorem fixture_logic_rows_replay_reference_semantics :
    Fixture.logicRows = projectedLogicRows := by
  decide

theorem fixture_gql_bag_has_three_rows : Fixture.gqlRows.length = 3 := by
  decide

theorem fixture_gql_bag_preserves_parallel_witnesses :
    Fixture.gqlRows = [("1", "2"), ("1", "2"), ("1", "3")] := by
  rfl

theorem fixture_logic_bag_has_three_rows : Fixture.logicRows.length = 3 := by
  decide

theorem fixture_logic_bag_preserves_parallel_witnesses :
    Fixture.logicRows = [("1", "2", 10), ("1", "2", 10), ("1", "3", 11)] := by
  rfl

theorem fixture_policy_is_canonical :
    Fixture.multiplicity = "bag-by-witness-edge" ∧
      Fixture.ordering = "canonical-row-json" := by
  decide

theorem committed_artifact_replay_is_admitted : replayAdmitted committedCheck = true := by
  decide

theorem payload_tampering_is_rejected : replayAdmitted tamperedPayloadCheck = false := by
  decide

end ASPProof.PolyglotSearchArtifactReplay
