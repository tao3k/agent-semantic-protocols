import ASPProof.HookEnforcementKernel

namespace ASPProof.HookPlatformActionProjection

open ASPProof.HookEnforcementKernel

inductive HostInvocationKind where
  | read
  | edit
  | execute
  | search
  | mcp
  | enumerate
  | unknown
  deriving BEq, DecidableEq, Repr

inductive SemanticCapability where
  | read
  | edit
  | execute
  | search
  | enumerate
  deriving BEq, DecidableEq, Repr

structure PlatformEnvelope where
  hostInvocation : HostInvocationKind
  semanticCapabilities : List SemanticCapability
  profiles : List String
  subjects : List String
  hookEventObserved : Bool
  deriving DecidableEq, Repr

structure ActionIR where
  hostInvocation : HostInvocationKind
  semanticCapabilities : List SemanticCapability
  profiles : List String
  subjects : List String
  deriving DecidableEq, Repr

structure Rule where
  hostInvocationsAny : List HostInvocationKind
  semanticCapabilitiesAny : List SemanticCapability
  profilesAny : List String
  decision : Decision
  deriving DecidableEq, Repr

def project (envelope : PlatformEnvelope) : Option ActionIR :=
  if envelope.hookEventObserved then
    some {
      hostInvocation := envelope.hostInvocation
      semanticCapabilities := envelope.semanticCapabilities
      profiles := envelope.profiles
      subjects := envelope.subjects
    }
  else
    none

def hostMatches (rule : Rule) (action : ActionIR) : Bool :=
  rule.hostInvocationsAny.isEmpty || rule.hostInvocationsAny.contains action.hostInvocation

def semanticMatches (rule : Rule) (action : ActionIR) : Bool :=
  rule.semanticCapabilitiesAny.isEmpty ||
    rule.semanticCapabilitiesAny.any action.semanticCapabilities.contains

def profileMatches (rule : Rule) (action : ActionIR) : Bool :=
  rule.profilesAny.isEmpty || rule.profilesAny.any action.profiles.contains

def ruleMatches (rule : Rule) (action : ActionIR) : Bool :=
  (hostMatches rule action && semanticMatches rule action) && profileMatches rule action

def applyRule (rule : Rule) (action : ActionIR) : Option Decision :=
  if ruleMatches rule action then some rule.decision else none

def dominantDecision : Decision → Decision → Decision
  | .deny, _ => .deny
  | _, .deny => .deny
  | .block, _ => .block
  | _, .block => .block
  | .allow, .allow => .allow

def mergeDecision : Option Decision → Option Decision → Option Decision
  | none, decision => decision
  | decision, none => decision
  | some left, some right => some (dominantDecision left right)

def evaluateRules (rules : List Rule) (action : ActionIR) : Option Decision :=
  rules.foldl (fun decision rule => mergeDecision decision (applyRule rule action)) none

def evaluate (envelope : PlatformEnvelope) (rules : List Rule) : Option Decision :=
  (project envelope).bind (evaluateRules rules)

def HostFailClosed (envelope : PlatformEnvelope) (executed : Bool) : Prop :=
  envelope.hookEventObserved = false → executed = false

def withSubjects (action : ActionIR) (subjects : List String) : ActionIR :=
  { action with subjects := subjects }

def semanticEffect : SemanticCapability → Effect
  | .read => .sourceRead
  | .edit => .sourcePatch
  | .execute => .sourceExecute
  | .search => .sourceSearch
  | .enumerate => .unknownSourceLike

def hostFallbackEffect : HostInvocationKind → Effect
  | .read => .sourceRead
  | .edit => .sourcePatch
  | .execute => .sourceExecute
  | .search => .sourceSearch
  | .mcp | .enumerate | .unknown => .unknownSourceLike

def toKernelAction (action : ActionIR) : AgentAction :=
  { effect :=
      match action.semanticCapabilities with
      | capability :: _ => semanticEffect capability
      | [] => hostFallbackEffect action.hostInvocation }

theorem subjects_do_not_create_semantic_capabilities
    (action : ActionIR)
    (subjects : List String) :
    (withSubjects action subjects).semanticCapabilities = action.semanticCapabilities := by
  rfl

theorem subjects_do_not_change_host_invocation
    (action : ActionIR)
    (subjects : List String) :
    (withSubjects action subjects).hostInvocation = action.hostInvocation := by
  rfl

theorem rule_axes_are_conjunctive (rule : Rule) (action : ActionIR) :
    ruleMatches rule action =
      ((hostMatches rule action && semanticMatches rule action) && profileMatches rule action) := by
  rfl

theorem unobserved_envelope_has_no_action_ir
    (envelope : PlatformEnvelope)
    (unobserved : envelope.hookEventObserved = false) :
    project envelope = none := by
  simp [project, unobserved]

theorem unobserved_envelope_has_no_policy_decision
    (envelope : PlatformEnvelope)
    (rules : List Rule)
    (unobserved : envelope.hookEventObserved = false) :
    evaluate envelope rules = none := by
  simp [evaluate, project, unobserved]

theorem deny_dominates_left (decision : Decision) :
    dominantDecision .deny decision = .deny := by
  cases decision <;> rfl

theorem deny_dominates_right (decision : Decision) :
    dominantDecision decision .deny = .deny := by
  cases decision <;> rfl

theorem fail_closed_unobserved_action_does_not_execute
    (envelope : PlatformEnvelope)
    (executed : Bool)
    (failClosed : HostFailClosed envelope executed)
    (unobserved : envelope.hookEventObserved = false) :
    executed = false :=
  failClosed unobserved

end ASPProof.HookPlatformActionProjection
