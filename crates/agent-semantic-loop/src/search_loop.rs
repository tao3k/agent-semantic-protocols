// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use agent_semantic_context_product::ActiveProgram;
use agent_semantic_context_product::Digest;
use agent_semantic_context_product::ExecutionAuthority;
use agent_semantic_context_product::JoinPolicy;
use agent_semantic_context_product::ProtocolId;
use agent_semantic_context_product::RouteExecutionMode;
use agent_semantic_context_product::RouteProgram;
use agent_semantic_context_product::UncheckedContextProductStateV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StageExecutionStatus {
    Pending,
    Admitted {
        admission_ref: ProtocolId,
    },
    Granted {
        grant_ref: ProtocolId,
    },
    Running {
        attempt_ref: ProtocolId,
    },
    Succeeded {
        result_receipt_ref: ProtocolId,
    },
    Failed {
        failure_receipt_ref: ProtocolId,
    },
    Cancelled {
        cancellation_receipt_ref: ProtocolId,
    },
}

impl StageExecutionStatus {
    fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Admitted { .. } | Self::Granted { .. } | Self::Running { .. }
        )
    }

    fn is_succeeded(&self) -> bool {
        matches!(self, Self::Succeeded { .. })
    }

    fn result_receipt_ref(&self) -> Option<&ProtocolId> {
        match self {
            Self::Succeeded { result_receipt_ref } => Some(result_receipt_ref),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchLoopSnapshot {
    pub program_id: ProtocolId,
    pub program_digest: Digest,
    pub stages: BTreeMap<ProtocolId, StageExecutionStatus>,
    pub joined_group_ids: BTreeSet<ProtocolId>,
}

impl SearchLoopSnapshot {
    pub fn from_authoritative_state(
        state: &UncheckedContextProductStateV1,
    ) -> Result<Self, SearchLoopError> {
        let ActiveProgram::Admitted {
            program_id,
            program_digest,
            program,
            ..
        } = &state.active_program
        else {
            return Err(SearchLoopError::ProgramBinding);
        };
        let mut stages = program
            .stages
            .iter()
            .map(|stage| (stage.stage_id.clone(), StageExecutionStatus::Pending))
            .collect::<BTreeMap<_, _>>();
        for execution in &state.executions {
            let status = match execution {
                ExecutionAuthority::Admitted(_) => StageExecutionStatus::Admitted {
                    admission_ref: execution
                        .admission_id()
                        .expect("admitted authority has admission id")
                        .clone(),
                },
                ExecutionAuthority::Granted(_) => StageExecutionStatus::Granted {
                    grant_ref: execution
                        .grant_id()
                        .expect("granted authority has grant id")
                        .clone(),
                },
                ExecutionAuthority::InFlight(_) => StageExecutionStatus::Running {
                    attempt_ref: execution
                        .attempt_id()
                        .expect("in-flight authority has attempt id")
                        .clone(),
                },
                ExecutionAuthority::Consumed(_) => StageExecutionStatus::Succeeded {
                    result_receipt_ref: execution
                        .result_receipt_ref()
                        .expect("consumed authority has result receipt")
                        .clone(),
                },
                ExecutionAuthority::Revoked(_) => StageExecutionStatus::Cancelled {
                    cancellation_receipt_ref: execution
                        .reason_code()
                        .expect("revoked authority has reason code")
                        .clone(),
                },
            };
            for stage_id in execution.stage_ids() {
                let Some(stage_status) = stages.get_mut(stage_id) else {
                    return Err(SearchLoopError::StageSet);
                };
                *stage_status = status.clone();
            }
        }
        Ok(Self {
            program_id: program_id.clone(),
            program_digest: program_digest.clone(),
            stages,
            joined_group_ids: state
                .joined_execution_groups
                .iter()
                .map(|joined| joined.execution_group_id.clone())
                .collect(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchLoopDirective {
    AdmitSerial {
        group_id: ProtocolId,
        stage_id: ProtocolId,
    },
    AdmitBatch {
        group_id: ProtocolId,
        stage_ids: Vec<ProtocolId>,
        batch_capability_ref: ProtocolId,
    },
    AdmitParallel {
        group_id: ProtocolId,
        stage_ids: Vec<ProtocolId>,
        max_parallel: u64,
        independence_proof_ref: ProtocolId,
    },
    Wait {
        group_id: ProtocolId,
        active_stage_ids: Vec<ProtocolId>,
    },
    EvaluateJoin {
        group_id: ProtocolId,
        policy: JoinPolicy,
        result_receipt_refs: Vec<ProtocolId>,
    },
    EvaluateClosure,
    Blocked {
        group_id: ProtocolId,
        failed_stage_ids: Vec<ProtocolId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchLoopError {
    ProgramBinding,
    StageSet,
    JoinedGroup,
    ExecutionGroup,
    Dependency,
}

impl fmt::Display for SearchLoopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ProgramBinding => "search loop snapshot does not bind the route program",
            Self::StageSet => "search loop snapshot stage set does not match the route program",
            Self::JoinedGroup => "search loop snapshot references an unknown joined group",
            Self::ExecutionGroup => "search loop execution group is invalid",
            Self::Dependency => "search loop has no runnable stage for an unresolved dependency",
        })
    }
}

impl std::error::Error for SearchLoopError {}

pub struct SearchLoopReducer;

impl SearchLoopReducer {
    pub fn advance(
        program: &RouteProgram,
        snapshot: &SearchLoopSnapshot,
    ) -> Result<SearchLoopDirective, SearchLoopError> {
        validate_snapshot(program, snapshot)?;

        let predecessors = predecessor_map(program);
        for group in &program.execution_groups {
            if snapshot.joined_group_ids.contains(&group.group_id) {
                continue;
            }

            let failed_stage_ids: Vec<_> = group
                .stage_ids
                .iter()
                .filter(|stage_id| {
                    matches!(
                        snapshot.stages.get(*stage_id),
                        Some(
                            StageExecutionStatus::Failed { .. }
                                | StageExecutionStatus::Cancelled { .. }
                        )
                    )
                })
                .cloned()
                .collect();
            if !failed_stage_ids.is_empty() {
                return Ok(SearchLoopDirective::Blocked {
                    group_id: group.group_id.clone(),
                    failed_stage_ids,
                });
            }

            if group.stage_ids.iter().all(|stage_id| {
                snapshot
                    .stages
                    .get(stage_id)
                    .is_some_and(StageExecutionStatus::is_succeeded)
            }) {
                let mut result_receipt_refs = group
                    .stage_ids
                    .iter()
                    .filter_map(|stage_id| {
                        snapshot
                            .stages
                            .get(stage_id)
                            .and_then(StageExecutionStatus::result_receipt_ref)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                result_receipt_refs.sort();
                result_receipt_refs.dedup();
                return Ok(SearchLoopDirective::EvaluateJoin {
                    group_id: group.group_id.clone(),
                    policy: group.join_policy,
                    result_receipt_refs,
                });
            }

            let active_stage_ids: Vec<_> = group
                .stage_ids
                .iter()
                .filter(|stage_id| {
                    snapshot
                        .stages
                        .get(*stage_id)
                        .is_some_and(StageExecutionStatus::is_active)
                })
                .cloned()
                .collect();
            let ready_stage_ids: Vec<_> = group
                .stage_ids
                .iter()
                .filter(|stage_id| {
                    matches!(
                        snapshot.stages.get(*stage_id),
                        Some(StageExecutionStatus::Pending)
                    ) && predecessors
                        .get(*stage_id)
                        .into_iter()
                        .flatten()
                        .all(|predecessor| {
                            snapshot
                                .stages
                                .get(*predecessor)
                                .is_some_and(StageExecutionStatus::is_succeeded)
                        })
                })
                .cloned()
                .collect();

            return match group.mode {
                RouteExecutionMode::Serial => {
                    if let Some(stage_id) = ready_stage_ids.into_iter().next() {
                        if active_stage_ids.is_empty() {
                            Ok(SearchLoopDirective::AdmitSerial {
                                group_id: group.group_id.clone(),
                                stage_id,
                            })
                        } else {
                            Ok(SearchLoopDirective::Wait {
                                group_id: group.group_id.clone(),
                                active_stage_ids,
                            })
                        }
                    } else if active_stage_ids.is_empty() {
                        Err(SearchLoopError::Dependency)
                    } else {
                        Ok(SearchLoopDirective::Wait {
                            group_id: group.group_id.clone(),
                            active_stage_ids,
                        })
                    }
                }
                RouteExecutionMode::Batch => {
                    if !active_stage_ids.is_empty() {
                        Ok(SearchLoopDirective::Wait {
                            group_id: group.group_id.clone(),
                            active_stage_ids,
                        })
                    } else if ready_stage_ids.len() != group.stage_ids.len() {
                        Err(SearchLoopError::Dependency)
                    } else {
                        Ok(SearchLoopDirective::AdmitBatch {
                            group_id: group.group_id.clone(),
                            stage_ids: ready_stage_ids,
                            batch_capability_ref: group
                                .batch_capability_ref
                                .clone()
                                .ok_or(SearchLoopError::ExecutionGroup)?,
                        })
                    }
                }
                RouteExecutionMode::Parallel => {
                    let available = group
                        .max_parallel
                        .saturating_sub(active_stage_ids.len() as u64)
                        as usize;
                    let stage_ids: Vec<_> = ready_stage_ids.into_iter().take(available).collect();
                    if stage_ids.is_empty() {
                        if active_stage_ids.is_empty() {
                            Err(SearchLoopError::Dependency)
                        } else {
                            Ok(SearchLoopDirective::Wait {
                                group_id: group.group_id.clone(),
                                active_stage_ids,
                            })
                        }
                    } else {
                        Ok(SearchLoopDirective::AdmitParallel {
                            group_id: group.group_id.clone(),
                            stage_ids,
                            max_parallel: group.max_parallel,
                            independence_proof_ref: group
                                .independence_proof_ref
                                .clone()
                                .ok_or(SearchLoopError::ExecutionGroup)?,
                        })
                    }
                }
            };
        }
        Ok(SearchLoopDirective::EvaluateClosure)
    }
}

fn validate_snapshot(
    program: &RouteProgram,
    snapshot: &SearchLoopSnapshot,
) -> Result<(), SearchLoopError> {
    if snapshot.program_id != program.program_id
        || snapshot.program_digest != program.program_digest
    {
        return Err(SearchLoopError::ProgramBinding);
    }
    let program_stage_ids: BTreeSet<_> =
        program.stages.iter().map(|stage| &stage.stage_id).collect();
    let snapshot_stage_ids: BTreeSet<_> = snapshot.stages.keys().collect();
    if program_stage_ids != snapshot_stage_ids {
        return Err(SearchLoopError::StageSet);
    }
    let group_ids: BTreeSet<_> = program
        .execution_groups
        .iter()
        .map(|group| &group.group_id)
        .collect();
    if snapshot
        .joined_group_ids
        .iter()
        .any(|group_id| !group_ids.contains(group_id))
    {
        return Err(SearchLoopError::JoinedGroup);
    }
    Ok(())
}

fn predecessor_map(program: &RouteProgram) -> BTreeMap<&ProtocolId, Vec<&ProtocolId>> {
    let mut predecessors = BTreeMap::new();
    for edge in &program.edges {
        predecessors
            .entry(&edge.to)
            .or_insert_with(Vec::new)
            .push(&edge.from);
    }
    predecessors
}

#[cfg(test)]
#[path = "../tests/unit/search_loop.rs"]
mod tests;
