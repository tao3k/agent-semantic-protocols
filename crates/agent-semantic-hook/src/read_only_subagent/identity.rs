//! Typed resident-child identity and authorization context.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentEnabled(bool);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagedChildName<'a>(&'a str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfiguredCodexAgentName<'a>(&'a str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfiguredResidentRole<'a>(&'a str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodexHookAgentId<'a>(&'a str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodexHookAgentType<'a>(&'a str);

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ResidentChildIdentityProof {
    CodexHookPayloadLiveTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentChildSessionId<'a>(&'a str);

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ResidentIdentityStatus {
    LiveTargetVerified,
    Unverified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentSandboxMode<'a>(&'a str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentRootSessionId<'a>(&'a str);

impl ResidentEnabled {
    pub fn new(enabled: bool) -> Self {
        Self(enabled)
    }
}

macro_rules! non_empty_borrowed_identity {
    ($type_name:ident) => {
        impl<'a> $type_name<'a> {
            pub fn new(value: &'a str) -> Option<Self> {
                (!value.trim().is_empty()).then_some(Self(value))
            }

            pub fn as_str(self) -> &'a str {
                self.0
            }
        }
    };
}

non_empty_borrowed_identity!(ManagedChildName);
non_empty_borrowed_identity!(ConfiguredCodexAgentName);
non_empty_borrowed_identity!(ConfiguredResidentRole);
non_empty_borrowed_identity!(CodexHookAgentId);
non_empty_borrowed_identity!(CodexHookAgentType);
non_empty_borrowed_identity!(ResidentChildSessionId);
non_empty_borrowed_identity!(ResidentSandboxMode);
non_empty_borrowed_identity!(ResidentRootSessionId);

impl ResidentChildIdentityProof {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CodexHookPayloadLiveTarget => "codex-hook-payload-live-target",
        }
    }
}

impl ResidentIdentityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LiveTargetVerified => "live-target-verified",
            Self::Unverified => "unverified",
        }
    }
}

pub struct HookSubagentPermissionContext<'a> {
    resident_enabled: ResidentEnabled,
    managed_child_name: ManagedChildName<'a>,
    configured_codex_agent_name: ConfiguredCodexAgentName<'a>,
    configured_role: ConfiguredResidentRole<'a>,
    codex_hook_agent_id: Option<CodexHookAgentId<'a>>,
    codex_hook_agent_type: Option<CodexHookAgentType<'a>>,
    resident_child_identity_proof: Option<ResidentChildIdentityProof>,
    resident_child_session_id: Option<ResidentChildSessionId<'a>>,
    identity_status: ResidentIdentityStatus,
    sandbox_mode: Option<ResidentSandboxMode<'a>>,
    session_id: ResidentRootSessionId<'a>,
}

impl HookSubagentPermissionContext<'_> {
    #[allow(clippy::too_many_arguments)]
    pub fn new<'a>(
        resident_enabled: ResidentEnabled,
        managed_child_name: ManagedChildName<'a>,
        configured_codex_agent_name: ConfiguredCodexAgentName<'a>,
        configured_role: ConfiguredResidentRole<'a>,
        codex_hook_agent_id: Option<CodexHookAgentId<'a>>,
        codex_hook_agent_type: Option<CodexHookAgentType<'a>>,
        resident_child_identity_proof: Option<ResidentChildIdentityProof>,
        resident_child_session_id: Option<ResidentChildSessionId<'a>>,
        identity_status: ResidentIdentityStatus,
        sandbox_mode: Option<ResidentSandboxMode<'a>>,
        session_id: ResidentRootSessionId<'a>,
    ) -> HookSubagentPermissionContext<'a> {
        HookSubagentPermissionContext {
            resident_enabled,
            managed_child_name,
            configured_codex_agent_name,
            configured_role,
            codex_hook_agent_id,
            codex_hook_agent_type,
            resident_child_identity_proof,
            resident_child_session_id,
            identity_status,
            sandbox_mode,
            session_id,
        }
    }

    pub fn resident_enabled(&self) -> bool {
        self.resident_enabled.0
    }

    pub(super) fn managed_child_name(&self) -> &str {
        self.managed_child_name.0
    }

    pub(super) fn configured_codex_agent_name(&self) -> &str {
        self.configured_codex_agent_name.0
    }

    pub(super) fn configured_role(&self) -> &str {
        self.configured_role.0
    }

    pub(super) fn codex_hook_agent_id(&self) -> Option<&str> {
        self.codex_hook_agent_id.map(|value| value.0)
    }

    pub fn codex_hook_agent_type(&self) -> Option<&str> {
        self.codex_hook_agent_type.map(|value| value.0)
    }

    pub fn resident_child_identity_proof(&self) -> Option<&str> {
        self.resident_child_identity_proof
            .map(ResidentChildIdentityProof::as_str)
    }

    pub fn resident_child_session_id(&self) -> Option<&str> {
        self.resident_child_session_id.map(|value| value.0)
    }

    pub(super) fn identity_status(&self) -> &str {
        self.identity_status.as_str()
    }

    pub(super) fn sandbox_mode(&self) -> Option<&str> {
        self.sandbox_mode.map(|value| value.0)
    }

    pub(super) fn is_read_only_sandbox(&self) -> bool {
        self.sandbox_mode().is_some_and(|mode| {
            let normalized = mode.trim().to_ascii_lowercase();
            normalized == "read-only" || normalized == "readonly"
        })
    }

    pub(super) fn session_id(&self) -> &str {
        self.session_id.0
    }

    /// Authorize a configured resident from stable configuration plus the live
    /// hook identity. `canonicalTarget` is deliberately absent: it is a
    /// dispatch hint, not authorization evidence.
    pub fn resident_authorized(&self) -> bool {
        self.resident_enabled.0
            && !self.configured_codex_agent_name.0.trim().is_empty()
            && !self.configured_role.0.trim().is_empty()
            && self
                .codex_hook_agent_id
                .is_some_and(|agent_id| !agent_id.0.trim().is_empty())
            && self
                .codex_hook_agent_type
                .is_some_and(|live_type| live_type.0 == self.configured_role.0)
            && self.resident_child_identity_proof.is_some_and(|proof| {
                proof == ResidentChildIdentityProof::CodexHookPayloadLiveTarget
            })
            && self
                .resident_child_session_id
                .is_some_and(|child_session| child_session.0 == self.session_id.0)
    }
}
