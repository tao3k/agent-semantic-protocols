-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.MultiAgentProviderFacadeAuthority

structure RegisteredProviderSet where
  containsBinaryDigest : Nat → Bool
  containsLanguageDigest : Nat → Bool

inductive SessionBindingEvidence where
  | nonAsp
  | exactBound
  | unavailableForAspSession
  deriving DecidableEq, Repr

inductive ProviderEntryPoint where
  | aspFacade (languageDigest : Nat)
  | directBinary (binaryDigest : Nat)
  deriving DecidableEq, Repr

def providerInvocationAuthorized
    (providers : RegisteredProviderSet)
    (binding : SessionBindingEvidence)
    (entryPoint : ProviderEntryPoint) : Bool :=
  match entryPoint with
  | .aspFacade languageDigest =>
      providers.containsLanguageDigest languageDigest
  | .directBinary binaryDigest =>
      match binding with
      | .nonAsp => !providers.containsBinaryDigest binaryDigest
      | .exactBound | .unavailableForAspSession => false

theorem every_registered_provider_binary_is_denied_in_bound_session
    (providers : RegisteredProviderSet)
    (binaryDigest : Nat)
    (_registered : providers.containsBinaryDigest binaryDigest = true) :
    providerInvocationAuthorized providers .exactBound (.directBinary binaryDigest) = false := by
  rfl

theorem missing_child_binding_fails_closed_for_direct_provider_execution
    (providers : RegisteredProviderSet)
    (binaryDigest : Nat) :
    providerInvocationAuthorized providers .unavailableForAspSession
      (.directBinary binaryDigest) = false := by
  rfl

theorem registered_language_facade_is_authorized_independently_of_language
    (providers : RegisteredProviderSet)
    (binding : SessionBindingEvidence)
    (languageDigest : Nat)
    (registered : providers.containsLanguageDigest languageDigest = true) :
    providerInvocationAuthorized providers binding (.aspFacade languageDigest) = true := by
  simpa [providerInvocationAuthorized] using registered

theorem unregistered_binary_is_not_misclassified_as_provider_internal
    (providers : RegisteredProviderSet)
    (binaryDigest : Nat)
    (unregistered : providers.containsBinaryDigest binaryDigest = false) :
    providerInvocationAuthorized providers .nonAsp (.directBinary binaryDigest) = true := by
  simp [providerInvocationAuthorized, unregistered]

end ASPProof.MultiAgentProviderFacadeAuthority
