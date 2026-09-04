use serde::Deserialize;
use serde::Serialize;

macro_rules! client_identifier {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, String> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(concat!(stringify!($name), " must be non-empty").to_owned());
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = String;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
    };
}

client_identifier!(ClientProjectId);
client_identifier!(ClientWorkspaceIdentity);
client_identifier!(ClientSessionId);
client_identifier!(ClientRequestId);
client_identifier!(ClientSchemaId);
client_identifier!(ClientRouteId);
client_identifier!(ClientReasonKind);
client_identifier!(AgentRootSessionId);
client_identifier!(AgentParentThreadId);
client_identifier!(AgentChildThreadId);
client_identifier!(AgentName);
client_identifier!(AgentPath);
client_identifier!(AgentRouteKey);
