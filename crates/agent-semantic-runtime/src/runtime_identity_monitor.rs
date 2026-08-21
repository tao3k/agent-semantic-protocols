//! Runtime-owned identity observation state machine.

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MonitorAction {
    Noop,
    BeginDrain,
    SpawnLatest,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeIdentityMonitor {
    observed: Option<String>,
    draining: bool,
    latest: Option<String>,
}

impl RuntimeIdentityMonitor {
    pub fn observe(&mut self, identity: impl Into<String>) -> MonitorAction {
        let identity = identity.into();
        if self.observed.as_deref() == Some(identity.as_str()) && !self.draining {
            return MonitorAction::Noop;
        }
        self.latest = Some(identity);
        if self.draining {
            MonitorAction::Noop
        } else {
            self.draining = true;
            MonitorAction::BeginDrain
        }
    }

    pub fn drain_completed(&mut self) -> MonitorAction {
        self.draining = false;
        if self.latest != self.observed {
            self.observed = self.latest.clone();
            MonitorAction::SpawnLatest
        } else {
            MonitorAction::Noop
        }
    }
}
