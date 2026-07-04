inductive IdentitySource where
  | selector
  | pathLine
deriving DecidableEq

inductive RendererDecision where
  | accept
  | reject
deriving DecidableEq

structure SearchPacket where
  selectorPresent : Bool
  identitySource : IdentitySource

def contractValid (p : SearchPacket) : Prop :=
  p.selectorPresent = true /\ p.identitySource = IdentitySource.selector

def producerBugPacket : SearchPacket :=
  { selectorPresent := false, identitySource := IdentitySource.pathLine }

def producerFixedPacket : SearchPacket :=
  { selectorPresent := true, identitySource := IdentitySource.selector }

def defensiveRenderer (p : SearchPacket) : RendererDecision :=
  match p.identitySource with
  | IdentitySource.selector => RendererDecision.accept
  | IdentitySource.pathLine => RendererDecision.accept

def selectorOnlyRenderer (p : SearchPacket) : RendererDecision :=
  match p.identitySource with
  | IdentitySource.selector =>
      if p.selectorPresent then RendererDecision.accept else RendererDecision.reject
  | IdentitySource.pathLine => RendererDecision.reject

def rendererCompliant (renderer : SearchPacket -> RendererDecision) : Prop :=
  forall p, renderer p = RendererDecision.accept -> contractValid p

theorem producer_bug_packet_invalid :
    Not (contractValid producerBugPacket) := by
  intro h
  cases h.left

theorem defensive_renderer_accepts_invalid_packet :
    defensiveRenderer producerBugPacket = RendererDecision.accept /\
      Not (contractValid producerBugPacket) := by
  constructor
  · rfl
  · exact producer_bug_packet_invalid

theorem defensive_renderer_not_compliant :
    Not (rendererCompliant defensiveRenderer) := by
  intro compliant
  have accepted : defensiveRenderer producerBugPacket = RendererDecision.accept := rfl
  exact producer_bug_packet_invalid (compliant producerBugPacket accepted)

theorem producer_fixed_packet_valid :
    contractValid producerFixedPacket := by
  constructor <;> rfl

theorem selector_only_renderer_compliant :
    rendererCompliant selectorOnlyRenderer := by
  intro p accepted
  cases p with
  | mk selectorPresent identitySource =>
      cases identitySource with
      | selector =>
          simp [selectorOnlyRenderer, contractValid] at accepted ⊢
          exact accepted
      | pathLine =>
          simp [selectorOnlyRenderer] at accepted
