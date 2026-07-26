use serde::{Deserialize, Serialize};

macro_rules! search_budget_value {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(u64);

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }

        impl $name {
            pub fn get(self) -> u64 {
                self.0
            }
        }
    };
}

search_budget_value!(SearchCommandLimit);
search_budget_value!(SearchElapsedTimeLimitMs);
search_budget_value!(SearchPacketSizeLimitBytes);
search_budget_value!(SearchChoiceDepthLimit);
search_budget_value!(SearchParallelismLimit);
search_budget_value!(SearchAggregateProviderLatencyLimitMs);
search_budget_value!(SearchParentVisibleSizeLimitBytes);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchBudget {
    pub max_commands: SearchCommandLimit,
    pub max_elapsed_ms: SearchElapsedTimeLimitMs,
    pub max_packet_bytes: SearchPacketSizeLimitBytes,
    pub max_choice_depth: SearchChoiceDepthLimit,
    pub max_parallel: SearchParallelismLimit,
    pub max_aggregate_provider_latency_ms: SearchAggregateProviderLatencyLimitMs,
    pub max_parent_visible_bytes: SearchParentVisibleSizeLimitBytes,
}
