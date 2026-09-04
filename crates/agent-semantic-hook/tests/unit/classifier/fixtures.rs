use agent_semantic_hook::HookRuntime;

pub(crate) fn registry() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    }
}

pub(crate) fn registry_without_providers() -> HookRuntime {
    registry()
}

pub(crate) fn rust_registry() -> HookRuntime {
    registry()
}

pub(crate) fn builtin_programming_runtime() -> HookRuntime {
    registry()
}
