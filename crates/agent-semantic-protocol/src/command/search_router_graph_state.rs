use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveGraphContinuationIdentity {
    pub session_id: String,
    pub node_id: String,
    pub snapshot_digest: String,
    pub state_digest: String,
}

impl InteractiveGraphContinuationIdentity {
    pub fn new(
        session_id: impl Into<String>,
        node_id: impl Into<String>,
        snapshot_digest: impl Into<String>,
        state_digest: impl Into<String>,
    ) -> Option<Self> {
        let session_id = session_id.into();
        let node_id = node_id.into();
        let snapshot_digest = snapshot_digest.into();
        let state_digest = state_digest.into();
        if session_id.trim().is_empty()
            || node_id.trim().is_empty()
            || snapshot_digest.trim().is_empty()
            || state_digest.trim().is_empty()
        {
            return None;
        }
        Some(Self {
            session_id,
            node_id,
            snapshot_digest,
            state_digest,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveGraphHopAccounting {
    pub semantic_graph_hops: u32,
    pub executed_graph_hops: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphTurboInvocationAccounting {
    pub input_nodes: u64,
    pub input_edges: u64,
    pub visited_nodes: u64,
    pub visited_edges: u64,
    pub iterations: u64,
    pub candidate_paths: u64,
    pub wall_time_ms: u64,
    pub peak_memory_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphTurboAccounting {
    pub invocations: u32,
    pub input_nodes: u64,
    pub input_edges: u64,
    pub visited_nodes: u64,
    pub visited_edges: u64,
    pub iterations: u64,
    pub candidate_paths: u64,
    pub wall_time_ms: u64,
    pub peak_memory_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphTurboProposalStatus {
    Candidate,
    Proposed,
    Heuristic,
    Partial,
}

ascent::ascent! {
    pub struct BoundedAscentGraphClosure;
    relation edge(u32, u32);
    relation hop_budget(u32);
    relation reachable(u32, u32, u32);

    reachable(source, target, 1) <--
        edge(source, target),
        hop_budget(max_hops),
        if 1 <= *max_hops;

    reachable(source, target, hops + 1) <--
        reachable(source, middle, hops),
        edge(middle, target),
        hop_budget(max_hops),
        if *hops < *max_hops;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AscentClosureBudget {
    pub max_input_edges: usize,
    pub max_graph_hops: u32,
    pub max_derived_facts: usize,
    pub max_rule_firing_bound: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AscentClosureReceipt {
    pub reachable: Vec<(u32, u32, u32)>,
    pub input_edges: usize,
    pub derived_facts: usize,
    pub rule_firing_bound: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AscentClosureError {
    InputEdgeBudgetExceeded,
    RuleFiringBoundExceeded,
    DerivedFactBudgetExceeded,
    ArithmeticOverflow,
}

pub fn run_bounded_ascent_graph_closure(
    edges: Vec<(u32, u32)>,
    budget: AscentClosureBudget,
) -> Result<AscentClosureReceipt, AscentClosureError> {
    if edges.len() > budget.max_input_edges {
        return Err(AscentClosureError::InputEdgeBudgetExceeded);
    }

    let rule_firing_bound = edges
        .len()
        .checked_mul(budget.max_derived_facts)
        .and_then(|recursive| recursive.checked_add(edges.len()))
        .ok_or(AscentClosureError::ArithmeticOverflow)?;
    if rule_firing_bound > budget.max_rule_firing_bound {
        return Err(AscentClosureError::RuleFiringBoundExceeded);
    }

    let input_edges = edges.len();
    let mut program = BoundedAscentGraphClosure::default();
    program.edge = edges;
    program.hop_budget = vec![(budget.max_graph_hops,)];
    program.run();

    let mut reachable = program.reachable;
    reachable.sort_unstable();
    reachable.dedup();
    if reachable.len() > budget.max_derived_facts {
        return Err(AscentClosureError::DerivedFactBudgetExceeded);
    }

    Ok(AscentClosureReceipt {
        derived_facts: reachable.len(),
        reachable,
        input_edges,
        rule_firing_bound,
    })
}

impl InteractiveGraphHopAccounting {
    pub const fn initial() -> Self {
        Self {
            semantic_graph_hops: 0,
            executed_graph_hops: 0,
        }
    }

    pub fn observed(semantic_graph_hops: u32, executed_graph_hops: u32) -> Option<Self> {
        (executed_graph_hops <= semantic_graph_hops).then_some(Self {
            semantic_graph_hops,
            executed_graph_hops,
        })
    }

    pub fn after_graph_hop(self, executed: bool) -> Option<Self> {
        let semantic_graph_hops = self.semantic_graph_hops.checked_add(1)?;
        let executed_graph_hops = self.executed_graph_hops.checked_add(u32::from(executed))?;
        Self::observed(semantic_graph_hops, executed_graph_hops)
    }

    pub const fn cache_saved_graph_hops(self) -> u32 {
        self.semantic_graph_hops - self.executed_graph_hops
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveGraphSearchState {
    pub continuation: InteractiveGraphContinuationIdentity,
    pub hops: InteractiveGraphHopAccounting,
    #[serde(default)]
    pub tool_actions: u32,
    #[serde(default)]
    pub graph_turbo: GraphTurboAccounting,
}

impl GraphTurboAccounting {
    pub fn after_invocation(self, invocation: GraphTurboInvocationAccounting) -> Option<Self> {
        Some(Self {
            invocations: self.invocations.checked_add(1)?,
            input_nodes: self.input_nodes.checked_add(invocation.input_nodes)?,
            input_edges: self.input_edges.checked_add(invocation.input_edges)?,
            visited_nodes: self.visited_nodes.checked_add(invocation.visited_nodes)?,
            visited_edges: self.visited_edges.checked_add(invocation.visited_edges)?,
            iterations: self.iterations.checked_add(invocation.iterations)?,
            candidate_paths: self
                .candidate_paths
                .checked_add(invocation.candidate_paths)?,
            wall_time_ms: self.wall_time_ms.checked_add(invocation.wall_time_ms)?,
            peak_memory_bytes: self.peak_memory_bytes.max(invocation.peak_memory_bytes),
        })
    }
}

impl InteractiveGraphSearchState {
    pub fn start(continuation: InteractiveGraphContinuationIdentity) -> Self {
        Self {
            continuation,
            hops: InteractiveGraphHopAccounting::initial(),
            tool_actions: 0,
            graph_turbo: GraphTurboAccounting::default(),
        }
    }

    pub fn resume_at(self, continuation: InteractiveGraphContinuationIdentity) -> Self {
        Self {
            continuation,
            hops: self.hops,
            tool_actions: self.tool_actions,
            graph_turbo: self.graph_turbo,
        }
    }

    pub fn advance_to(
        self,
        continuation: InteractiveGraphContinuationIdentity,
        executed: bool,
    ) -> Option<Self> {
        Some(Self {
            continuation,
            hops: self.hops.after_graph_hop(executed)?,
            tool_actions: self.tool_actions,
            graph_turbo: self.graph_turbo,
        })
    }

    pub fn after_tool_action(self) -> Option<Self> {
        Some(Self {
            tool_actions: self.tool_actions.checked_add(1)?,
            ..self
        })
    }

    pub fn after_graph_turbo(self, invocation: GraphTurboInvocationAccounting) -> Option<Self> {
        Some(Self {
            tool_actions: self.tool_actions.checked_add(1)?,
            graph_turbo: self.graph_turbo.after_invocation(invocation)?,
            ..self
        })
    }
}
