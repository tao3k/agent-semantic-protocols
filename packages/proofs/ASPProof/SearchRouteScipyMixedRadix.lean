import Std

namespace ASPProof.SearchRouteScipyMixedRadix

structure RouteCost where
  hops : Nat
  interactionRounds : Nat
  tokens : Nat
  searchCacheMisses : Nat
  modelCacheMisses : Nat
  deriving DecidableEq, Repr

structure LowerBounds where
  rounds : Nat
  tokens : Nat
  searchCache : Nat
  modelCache : Nat
  deriving DecidableEq, Repr

def appendDigit (high base low : Nat) : Nat :=
  high * base + low

def cacheTail (cost : RouteCost) (bounds : LowerBounds) : Nat :=
  appendDigit cost.searchCacheMisses bounds.modelCache cost.modelCacheMisses

def tokenTail (cost : RouteCost) (bounds : LowerBounds) : Nat :=
  appendDigit cost.tokens (bounds.searchCache * bounds.modelCache)
    (cacheTail cost bounds)

def lowerTail (cost : RouteCost) (bounds : LowerBounds) : Nat :=
  appendDigit cost.interactionRounds
    (bounds.tokens * (bounds.searchCache * bounds.modelCache))
    (tokenTail cost bounds)

def lowerBase (bounds : LowerBounds) : Nat :=
  bounds.rounds * (bounds.tokens * (bounds.searchCache * bounds.modelCache))

def encode (cost : RouteCost) (bounds : LowerBounds) : Nat :=
  appendDigit cost.hops (lowerBase bounds) (lowerTail cost bounds)

def WithinLowerBounds (cost : RouteCost) (bounds : LowerBounds) : Prop :=
  cost.interactionRounds < bounds.rounds ∧
    cost.tokens < bounds.tokens ∧
    cost.searchCacheMisses < bounds.searchCache ∧
    cost.modelCacheMisses < bounds.modelCache

theorem appendDigit_lt_product
    {high low highBase lowBase : Nat}
    (highBound : high < highBase)
    (lowBound : low < lowBase) :
    appendDigit high lowBase low < highBase * lowBase := by
  unfold appendDigit
  calc
    high * lowBase + low < high * lowBase + lowBase :=
      Nat.add_lt_add_left lowBound _
    _ = (high + 1) * lowBase := (Nat.succ_mul high lowBase).symm
    _ ≤ highBase * lowBase :=
      Nat.mul_le_mul_right lowBase (Nat.succ_le_iff.mpr highBound)

theorem cacheTail_lt_base
    {cost : RouteCost} {bounds : LowerBounds}
    (searchBound : cost.searchCacheMisses < bounds.searchCache)
    (modelBound : cost.modelCacheMisses < bounds.modelCache) :
    cacheTail cost bounds < bounds.searchCache * bounds.modelCache := by
  exact appendDigit_lt_product searchBound modelBound

theorem tokenTail_lt_base
    {cost : RouteCost} {bounds : LowerBounds}
    (tokenBound : cost.tokens < bounds.tokens)
    (searchBound : cost.searchCacheMisses < bounds.searchCache)
    (modelBound : cost.modelCacheMisses < bounds.modelCache) :
    tokenTail cost bounds < bounds.tokens * (bounds.searchCache * bounds.modelCache) := by
  exact appendDigit_lt_product tokenBound
    (cacheTail_lt_base searchBound modelBound)

theorem lowerTail_lt_base
    {cost : RouteCost} {bounds : LowerBounds}
    (within : WithinLowerBounds cost bounds) :
    lowerTail cost bounds < lowerBase bounds := by
  rcases within with ⟨roundBound, tokenBound, searchBound, modelBound⟩
  unfold lowerTail lowerBase
  exact appendDigit_lt_product roundBound
    (tokenTail_lt_base tokenBound searchBound modelBound)

theorem appendDigit_priority
    {high₁ high₂ base low₁ low₂ : Nat}
    (highPriority : high₁ < high₂)
    (low₁Bound : low₁ < base) :
    appendDigit high₁ base low₁ < appendDigit high₂ base low₂ := by
  unfold appendDigit
  calc
    high₁ * base + low₁ < high₁ * base + base :=
      Nat.add_lt_add_left low₁Bound _
    _ = (high₁ + 1) * base := (Nat.succ_mul high₁ base).symm
    _ ≤ high₂ * base :=
      Nat.mul_le_mul_right base (Nat.succ_le_iff.mpr highPriority)
    _ ≤ high₂ * base + low₂ := Nat.le_add_right _ _

theorem fewer_hops_dominate_all_bounded_lower_costs
    {left right : RouteCost} {bounds : LowerBounds}
    (fewerHops : left.hops < right.hops)
    (leftWithin : WithinLowerBounds left bounds) :
    encode left bounds < encode right bounds := by
  exact appendDigit_priority fewerHops (lowerTail_lt_base leftWithin)

theorem fewer_rounds_dominate_bounded_token_and_cache_costs
    {left right : RouteCost} {bounds : LowerBounds}
    (sameHops : left.hops = right.hops)
    (fewerRounds : left.interactionRounds < right.interactionRounds)
    (leftTokenBound : left.tokens < bounds.tokens)
    (leftSearchBound : left.searchCacheMisses < bounds.searchCache)
    (leftModelBound : left.modelCacheMisses < bounds.modelCache) :
    encode left bounds < encode right bounds := by
  have lowerPriority : lowerTail left bounds < lowerTail right bounds :=
    appendDigit_priority fewerRounds
      (tokenTail_lt_base leftTokenBound leftSearchBound leftModelBound)
  unfold encode appendDigit
  rw [sameHops]
  exact Nat.add_lt_add_left lowerPriority _

def float64ExactIntegerCap : Nat := 9007199254740991

theorem first_unsafe_integer_exceeds_float64_exact_cap :
    float64ExactIntegerCap < 2 ^ 53 := by
  decide

def cacheExampleBounds : LowerBounds where
  rounds := 1
  tokens := 1
  searchCache := 2
  modelCache := 2

def modelMissOnly : RouteCost where
  hops := 1
  interactionRounds := 0
  tokens := 0
  searchCacheMisses := 0
  modelCacheMisses := 1

def searchMissOnly : RouteCost where
  hops := 1
  interactionRounds := 0
  tokens := 0
  searchCacheMisses := 1
  modelCacheMisses := 0

theorem search_cache_and_model_cache_are_distinct_digits :
    encode modelMissOnly cacheExampleBounds <
      encode searchMissOnly cacheExampleBounds := by
  decide

def pythonShorterHopBounds : LowerBounds where
  rounds := 3
  tokens := 201
  searchCache := 1
  modelCache := 1

def pythonDirectRoute : RouteCost where
  hops := 1
  interactionRounds := 1
  tokens := 100
  searchCacheMisses := 0
  modelCacheMisses := 0

def pythonLongerRoute : RouteCost where
  hops := 2
  interactionRounds := 2
  tokens := 0
  searchCacheMisses := 0
  modelCacheMisses := 0

theorem python_direct_route_encoding_is_904 :
    encode pythonDirectRoute pythonShorterHopBounds = 904 := by
  decide

theorem python_direct_route_beats_longer_route :
    encode pythonDirectRoute pythonShorterHopBounds <
      encode pythonLongerRoute pythonShorterHopBounds := by
  decide

def pythonRoundBounds : LowerBounds where
  rounds := 5
  tokens := 101
  searchCache := 1
  modelCache := 1

def pythonFastRoundRoute : RouteCost where
  hops := 2
  interactionRounds := 1
  tokens := 100
  searchCacheMisses := 0
  modelCacheMisses := 0

def pythonCheapTokenRoute : RouteCost where
  hops := 2
  interactionRounds := 3
  tokens := 0
  searchCacheMisses := 0
  modelCacheMisses := 0

theorem python_fast_round_route_encoding_is_1211 :
    encode pythonFastRoundRoute pythonRoundBounds = 1211 := by
  decide

theorem python_fast_round_route_beats_cheaper_token_route :
    encode pythonFastRoundRoute pythonRoundBounds <
      encode pythonCheapTokenRoute pythonRoundBounds := by
  decide

def pythonCacheBounds : LowerBounds where
  rounds := 1
  tokens := 1
  searchCache := 3
  modelCache := 3

def pythonSearchHitRoute : RouteCost where
  hops := 2
  interactionRounds := 0
  tokens := 0
  searchCacheMisses := 0
  modelCacheMisses := 1

def pythonModelHitRoute : RouteCost where
  hops := 2
  interactionRounds := 0
  tokens := 0
  searchCacheMisses := 1
  modelCacheMisses := 0

theorem python_search_hit_route_encoding_is_19 :
    encode pythonSearchHitRoute pythonCacheBounds = 19 := by
  decide

theorem python_search_hit_route_beats_search_miss_route :
    encode pythonSearchHitRoute pythonCacheBounds <
      encode pythonModelHitRoute pythonCacheBounds := by
  decide

end ASPProof.SearchRouteScipyMixedRadix
