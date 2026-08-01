use std::path::PathBuf;

std::thread_local! {
    static HOOK_RUNTIME_PROJECT_ROOT_OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

struct HookRuntimeProjectRootOverrideGuard {
    previous: Option<PathBuf>,
}

impl Drop for HookRuntimeProjectRootOverrideGuard {
    fn drop(&mut self) {
        HOOK_RUNTIME_PROJECT_ROOT_OVERRIDE.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

pub(super) fn with_hook_runtime_project_root_override<T>(
    project_root: PathBuf,
    run: impl FnOnce() -> T,
) -> T {
    let previous = HOOK_RUNTIME_PROJECT_ROOT_OVERRIDE.with(|slot| slot.replace(Some(project_root)));
    let _guard = HookRuntimeProjectRootOverrideGuard { previous };
    run()
}

pub(super) fn hook_runtime_project_root_override() -> Option<PathBuf> {
    HOOK_RUNTIME_PROJECT_ROOT_OVERRIDE.with(|slot| slot.borrow().clone())
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_project_root.rs"]
mod tests;
