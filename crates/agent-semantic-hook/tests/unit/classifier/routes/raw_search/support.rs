use agent_semantic_hook::HookRuntime;

pub(super) fn registry() -> HookRuntime {
    super::super::registry_with_rust_and_python()
}

pub(super) fn prefixed_registry() -> HookRuntime {
    super::super::registry_with_prefixed_python()
}
