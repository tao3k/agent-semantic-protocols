import Lean

namespace ASPProof.Audit.Core

open Lean Elab Term Meta

structure Target where
  name : Name
  theoremFamily : String
  rfcClauseIds : List String

private def sortedUniqueNames (names : Array Name) : List String :=
  (names.toList.map Name.toString).mergeSort (· ≤ ·) |>.eraseDups

private def jsonStrings (values : List String) : Json :=
  Json.arr (values.map Json.str).toArray

private def sortedUniqueStrings (values : List String) : List String :=
  values.mergeSort (· ≤ ·) |>.eraseDups

private def declarationKind : ConstantInfo → String
  | .axiomInfo _ => "axiom"
  | .defnInfo _ => "definition"
  | .thmInfo _ => "theorem"
  | .opaqueInfo _ => "opaque"
  | .quotInfo _ => "quotient"
  | .inductInfo _ => "inductive"
  | .ctorInfo _ => "constructor"
  | .recInfo _ => "recursor"

private def targetJson (target : Target) : TermElabM Json := do
  let env ← getEnv
  let some info := env.find? target.name
    | throwError "audit target declaration not found: {target.name}"
  let typeText ← withOptions (fun options =>
      options
        |>.setBool `pp.universes false
        |>.setBool `pp.all false) do
    return (← ppExpr info.type).pretty
  let axioms := sortedUniqueNames (← collectAxioms target.name)
  pure <| Json.mkObj [
    ("type", Json.str typeText),
    ("theoremFamily", Json.str target.theoremFamily),
    ("rfcClauseIds", jsonStrings (sortedUniqueStrings target.rfcClauseIds)),
    ("name", Json.str target.name.toString),
    ("kind", Json.str (declarationKind info)),
    ("hasSorryAx", Json.bool (axioms.any (·.contains "sorryAx"))),
    ("axioms", jsonStrings axioms)
  ]

def proofAuditJson
    (moduleName sourcePath : String)
    (targets : List Target) :
    TermElabM Json := do
  let declarations ← targets.mapM targetJson
  let axiomSets ← targets.mapM (fun target => collectAxioms target.name)
  let axiomFreeCount := axiomSets.countP Array.isEmpty
  let axiomNames := sortedUniqueNames (axiomSets.foldl (· ++ ·) #[])
  pure <| Json.mkObj [
    ("schemaId", Json.str "asp.lean-proof-audit.v1"),
    ("schemaVersion", Json.str "1"),
    ("leanVersion", Json.str versionString),
    ("proofPackage", Json.str "ASPProof"),
    ("module", Json.str moduleName),
    ("sourcePath", Json.str sourcePath),
    ("declarationCount", toJson declarations.length),
    ("axiomFreeDeclarationCount", toJson axiomFreeCount),
    ("axiomDependentDeclarationCount",
      toJson (declarations.length - axiomFreeCount)),
    ("declarations", Json.arr declarations.toArray),
    ("axiomInventory", jsonStrings axiomNames),
    ("hasSorryAx", Json.bool (axiomNames.any (·.contains "sorryAx")))
  ]

end ASPProof.Audit.Core
