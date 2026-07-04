use std::{ffi::OsString, path::Path, sync::Mutex};

static ASP_STATE_HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

pub(crate) struct IsolatedAspStateHome {
    _guard: std::sync::MutexGuard<'static, ()>,
    previous: Option<OsString>,
}

impl IsolatedAspStateHome {
    pub(crate) fn activate(root: &Path) -> Self {
        let guard = ASP_STATE_HOME_ENV_LOCK
            .lock()
            .expect("ASP_STATE_HOME env lock");
        let previous = std::env::var_os("ASP_STATE_HOME");
        let state_home = root.join(".agent-semantic-protocols-test-state");
        unsafe {
            std::env::set_var("ASP_STATE_HOME", &state_home);
        }
        Self {
            _guard: guard,
            previous,
        }
    }
}

impl Drop for IsolatedAspStateHome {
    fn drop(&mut self) {
        unsafe {
            if let Some(previous) = &self.previous {
                std::env::set_var("ASP_STATE_HOME", previous);
            } else {
                std::env::remove_var("ASP_STATE_HOME");
            }
        }
    }
}
