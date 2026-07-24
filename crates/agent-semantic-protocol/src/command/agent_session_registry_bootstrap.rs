//! Resident-slot classifier; `binding` owns target transitions, `reasoning`
//! owns evidence verdicts, and target absence precedes runtime readiness.

use std::path::Path;

use agent_semantic_client_db::agent_session_registry::{
    AgentSessionLookupRequest, AgentSessionRegistry, ResidentChildBootstrapMenuInput,
    agent_session_unix_timestamp, resident_child_bootstrap_menu,
    resident_child_live_transport_gate, resident_child_runtime_repair_menu,
    resident_child_runtime_verified_menu,
};

use super::{SessionArgs, project_session_scope_id, resolved_root_session_id};
#[path = "agent_session_registry_binding.rs"]
pub(in crate::command) mod binding;
#[path = "agent_session_registry_bootstrap_parts/choice_construction.rs"]
mod choice_construction;
#[path = "agent_session_registry_observation.rs"]
mod observation;
#[path = "agent_session_registry_reasoning.rs"]
mod reasoning;
#[path = "agent_session_registry_bootstrap_render.rs"]
mod render;
#[path = "agent_session_registry_bootstrap_parts/runtime_repair.rs"]
mod runtime_repair;

pub(super) use self::choice_construction::codex_spawn_agent_metadata_capability;
use self::choice_construction::{
    canonical_resident_target, platform_native_create_action, registry_record_routable,
    reject_resident_child_bootstrap,
};
use self::runtime_repair::{
    latest_trace_result, main_agent_runtime_rebind_instruction, runtime_repair_diagnosis,
    runtime_switch_followup_message,
};
fn dispatch_shell_argument(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn bootstrap_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    project_root: &Path,
) -> Result<(), String> {
    let project_id = project_session_scope_id(registry, project_root)?;
    let name = args.name.as_deref().unwrap_or("asp-explore");
    reject_resident_child_bootstrap(registry, &project_id, name)?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let now = agent_session_unix_timestamp()?;
    let host_typed_spawn_observation = root_session_id
        .as_deref()
        .map(|root_session_id| {
            super::agent_session_registry_host_capability::fresh_host_typed_spawn_observation(
                registry,
                root_session_id,
                name,
                now,
            )
        })
        .transpose()?
        .flatten();
    let host_resident_target_observation = root_session_id
        .as_deref()
        .map(|root_session_id| {
            super::agent_session_registry_host_capability::fresh_host_resident_target_observation(
                registry,
                root_session_id,
                name,
                now,
            )
        })
        .transpose()?
        .flatten();
    let host_resident_target_present = host_resident_target_observation
        .as_ref()
        .is_some_and(|observation| observation.target_status == "present");
    let host_resident_transport_verified =
        host_resident_target_observation
            .as_ref()
            .is_some_and(|observation| {
                observation.target_status == "present" && observation.identity_status == "verified"
            });
    let host_resident_target_unroutable = host_resident_target_observation
        .as_ref()
        .is_some_and(|observation| observation.target_status == "unroutable");
    if args.child_session_id.is_some()
        || args.message_target_id.is_some()
        || args.parent_session_id.is_some()
        || args.model.is_some()
        || args.status.is_some()
    {
        return Err(
            "agent session bootstrap does not accept child identity, message target, model, parent, or status receipts; use the host-native create/resume/stop action and let SubagentStart/SubagentStop update the registry"
                .to_string(),
        );
    }
    let mut record = if let Some(root_session_id) = root_session_id.as_deref() {
        registry.lookup_session(AgentSessionLookupRequest {
            project_id: &project_id,
            session_id: None,
            root_session_id: Some(root_session_id),
            name: Some(name),
        })?
    } else {
        None
    };
    if record
        .as_ref()
        .is_some_and(|session| matches!(session.status.as_str(), "archived" | "closed"))
    {
        record = None;
    }
    let mut rollout_history_status = "not-needed";
    let mut rollout_history_action = "none";
    let mut registry_routable = record.as_ref().is_some_and(|session| {
        registry_record_routable(
            session,
            root_session_id.as_deref(),
            host_resident_transport_verified,
            now,
        )
    });
    if record.is_none() {
        let preflight =
            super::agent_session_registry_profile::adopt_reusable_rollout_session_before_create(
                registry,
                &project_id,
                root_session_id.as_deref(),
                args,
                Some(name),
                record.as_ref().map(|session| session.session_id.as_str()),
                now,
            )?;
        rollout_history_status = preflight.status;
        rollout_history_action = preflight.action;
        if let Some(adopted) = preflight.record {
            record = Some(adopted);
        }
        registry_routable = record.as_ref().is_some_and(|session| {
            registry_record_routable(
                session,
                root_session_id.as_deref(),
                host_resident_transport_verified,
                now,
            )
        });
    } else if !registry_routable {
        rollout_history_status = "locked-existing-repair-candidate";
        rollout_history_action = "preserve-candidate-identity-until-host-classification";
    }
    if host_resident_target_unroutable {
        if let Some(session) = record.as_mut() {
            session.status = "orphan-risk".to_string();
            session.message_target_id = None;
        }
        registry_routable = false;
    }
    let expected_model =
        super::agent_session_registry_validation::expected_model_for_session_profile(
            record
                .as_ref()
                .map_or(name, |session| session.name.as_str()),
            record.as_ref().map_or("", |session| session.role.as_str()),
        )?;
    let expected_reasoning_effort =
        super::agent_session_registry_validation::expected_reasoning_effort_for_session_profile(
            record
                .as_ref()
                .map_or(name, |session| session.name.as_str()),
            record.as_ref().map_or("", |session| session.role.as_str()),
        )?;
    let platform =
        crate::command::agent_session_registry::active_platform().unwrap_or("{platform}");
    let expected_host_requirement =
        resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
            platform,
            name,
            root_session_id: root_session_id.as_deref(),
            record: record.as_ref(),
            expected_model: expected_model.as_deref(),
            expected_reasoning_effort: expected_reasoning_effort.as_deref(),
            rollout_history_status: Some(rollout_history_status),
            rollout_history_action: Some(rollout_history_action),
            now,
        })
        .host_requirement;
    let expected_agent_type = expected_host_requirement.managed_agent_kind.to_string();
    let expected_canonical_target = canonical_resident_target(&expected_host_requirement);
    let subagent_start_registered_child_id = record
        .as_ref()
        .filter(|session| {
            root_session_id.as_deref().is_some_and(|root_session_id| {
                reasoning::typed_subagent_start_binding_is_valid(
                    session,
                    root_session_id,
                    &expected_agent_type,
                    agent_semantic_client_db::agent_session_registry::agent_session_message_target_is_live_bound(
                        session,
                        root_session_id,
                    ),
                )
            })
        })
        .map(|session| session.session_id.clone());
    let profile_attestation_target_verified = host_resident_transport_verified;
    let (attested_child_id, attestation_source) = reasoning::profile_attestation_identity(
        record.as_ref(),
        subagent_start_registered_child_id.as_ref(),
        root_session_id.as_deref(),
        &expected_agent_type,
        &expected_canonical_target,
        profile_attestation_target_verified,
    )
    .unzip();
    // Bootstrap is a local state classifier. Runtime evidence is written by
    // lifecycle observation paths; bootstrap must never synchronously start a
    // Codex app-server or any other child process to manufacture missing facts.
    let fresh_same_child_runtime_observation = false;
    let host_runtime_override_blocked = false;
    let host_observed_model: Option<String> = None;
    let host_observed_reasoning_effort: Option<String> = None;
    let runtime_drift = root_session_id
        .as_deref()
        .map(|root_session_id| {
            agent_semantic_hook::latest_subagent_runtime_drift(project_root, root_session_id)
        })
        .transpose()?
        .flatten()
        .filter(|observation| {
            observation::matches_resident_slot(
                &expected_agent_type,
                record.as_ref().map(|session| session.session_id.as_str()),
                &observation.expected_agent_type,
                &observation.child_session_id,
            )
        });
    let mut runtime_verification_observation = root_session_id
        .as_deref()
        .map(|root_session_id| {
            agent_semantic_hook::latest_subagent_runtime_rebind_verified(
                project_root,
                root_session_id,
            )
        })
        .transpose()?
        .flatten()
        .filter(|observation| {
            observation::matches_resident_slot(
                &expected_agent_type,
                record.as_ref().map(|session| session.session_id.as_str()),
                &observation.expected_agent_type,
                &observation.child_session_id,
            )
        })
        .or_else(|| {
            let root_session_id = root_session_id.as_deref()?;
            let child_session_id = subagent_start_registered_child_id.as_ref()?;
            let observed_model = host_observed_model.as_ref()?;
            let observed_reasoning_effort = host_observed_reasoning_effort.as_ref()?;
            let expected_model = expected_model.as_ref()?;
            let expected_reasoning_effort = expected_reasoning_effort.as_ref()?;
            (observed_model == expected_model
                && observed_reasoning_effort == expected_reasoning_effort)
                .then(|| {
                    let (reasoning_evidence, reasoning_assessment) =
                        reasoning::direct_reasoning_receipt(
                            root_session_id,
                            child_session_id,
                            observed_reasoning_effort,
                        );
                    agent_semantic_hook::SubagentRuntimeRebindVerifiedObservation {
                        root_session_id: root_session_id.to_string(),
                        child_session_id: child_session_id.clone(),
                        observed_agent_type: expected_agent_type.clone(),
                        expected_agent_type: expected_agent_type.clone(),
                        previous_observed_model: None,
                        previous_observed_reasoning_effort: None,
                        observed_model: observed_model.clone(),
                        observed_reasoning_effort: Some(observed_reasoning_effort.clone()),
                        expected_model: expected_model.clone(),
                        expected_reasoning_effort: Some(expected_reasoning_effort.clone()),
                        reasoning_evidence,
                        reasoning_assessment,
                        observation_source: "codex-app-server-thread-resume-after-subagent-start",
                        observation_count: 1,
                    }
                })
        })
        .or_else(|| {
            reasoning::profile_attested_runtime_observation(
                root_session_id.as_deref(),
                attested_child_id.as_ref(),
                &expected_agent_type,
                record.as_ref().and_then(|record| record.model.as_ref()),
                expected_model.as_ref(),
                expected_reasoning_effort.as_ref(),
                profile_attestation_target_verified,
                attestation_source,
            )
        });
    let runtime_reasoning_from_host = runtime_verification_observation
        .as_ref()
        .is_some_and(|observation| observation.observed_reasoning_effort.is_none())
        && host_observed_reasoning_effort.is_some();
    if runtime_reasoning_from_host
        && let Some(observation) = runtime_verification_observation.as_mut()
    {
        observation.observed_reasoning_effort = host_observed_reasoning_effort.clone();
    }
    let runtime_reasoning_profile_attested = reasoning::profile_attestation_is_valid(
        runtime_verification_observation.as_ref(),
        &expected_agent_type,
        expected_reasoning_effort.as_ref(),
        attested_child_id.as_ref(),
        expected_model.as_ref(),
        profile_attestation_target_verified,
        runtime_drift.as_ref(),
    );
    let runtime_verification_matches_profile = runtime_verification_observation
        .as_ref()
        .is_some_and(|observation| {
            agent_semantic_client_db::agent_session_registry::typed_runtime_observation_matches_profile(
                &observation.observed_agent_type,
                &observation.expected_agent_type,
                &observation.observed_model,
                observation.observed_reasoning_effort.as_deref(),
                observation.observation_source,
                expected_model.as_deref(),
                expected_reasoning_effort.as_deref(),
            )
        }) || runtime_reasoning_profile_attested;
    let runtime_verification_evidence_incomplete = runtime_verification_observation.is_some()
        && !runtime_verification_matches_profile
        && runtime_drift.is_none();
    let runtime_verified = runtime_verification_observation
        .clone()
        .filter(|_| runtime_verification_matches_profile);

    if let (Some(root_session_id), Some(observation)) =
        (root_session_id.as_deref(), runtime_verified.as_ref())
        && host_resident_target_present
        && let Some(mut verified_record) = registry.lookup_session(AgentSessionLookupRequest {
            project_id: &project_id,
            session_id: Some(&observation.child_session_id),
            root_session_id: Some(root_session_id),
            name: Some(name),
        })?
    {
        verified_record.status = "active".to_string();
        registry_routable = registry_record_routable(
            &verified_record,
            Some(root_session_id),
            host_resident_transport_verified,
            now,
        );
        record = Some(verified_record);
    }

    let mut menu = resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform,
        name,
        root_session_id: root_session_id.as_deref(),
        record: record.as_ref(),
        expected_model: expected_model.as_deref(),
        expected_reasoning_effort: expected_reasoning_effort.as_deref(),
        rollout_history_status: Some(rollout_history_status),
        rollout_history_action: Some(rollout_history_action),
        now,
    });
    menu = binding::require_host_tree_audit(menu, host_resident_target_observation.is_none());
    if let Some(observation) = host_resident_target_observation.as_ref() {
        menu = agent_semantic_client_db::agent_session_registry::resident_child_host_tree_observation_menu(
            menu,
            &observation.target_status,
            host_typed_spawn_observation
                .as_ref()
                .map(|typed| typed.field_status.as_str()),
        );
    }
    if let Some(observation) = host_typed_spawn_observation.as_ref() {
        menu.choices
            .retain(|choice| choice.id != "audit-host-typed-spawn-schema");
        if observation.field_status == "absent" {
            menu.choices
                .retain(|choice| choice.id != "create-managed-resident-child-after-host-tree-miss");
        }
    }
    if profile_attestation_target_verified {
        if runtime_verified.is_some() {
            menu = resident_child_runtime_verified_menu(
                menu,
                registry_routable,
                host_resident_transport_verified,
            );
        } else if runtime_verification_evidence_incomplete {
            menu = agent_semantic_client_db::agent_session_registry::resident_child_runtime_evidence_incomplete_menu(menu);
        } else if let Some(observation) = runtime_drift.as_ref() {
            menu =
                resident_child_runtime_repair_menu(menu, observation.consecutive_observation_count);
        } else if registry_routable && host_runtime_override_blocked {
            menu = resident_child_runtime_repair_menu(menu, 2);
        }
    }
    menu = resident_child_live_transport_gate(menu, host_resident_transport_verified);
    let host_resident_target_registers_child = host_resident_target_observation
        .as_ref()
        .is_some_and(|observation| {
            observation.target_status == "present"
                && observation.identity_status == "verified"
                && registry_routable
        });
    match (args.receipt_kind.as_deref(), args.command_json.as_deref()) {
        (Some(receipt_kind), Some(command_json)) => {
            let argv = serde_json::from_str::<Vec<String>>(command_json).map_err(|error| {
                format!("--command-json must encode an argv string array: {error}")
            })?;
            super::agent_session_registry_dispatch::validate_exact_argv(&argv)?;
            let canonical_command_json = serde_json::to_string(&argv)
                .map_err(|error| format!("failed to encode canonical dispatch argv: {error}"))?;
            if format!("{:?}", menu.state) == "Ready"
                && let Some(choice) = menu
                    .choices
                    .iter_mut()
                    .find(|choice| choice.id == "send-denied-asp-command")
            {
                let mut command = format!(
                    "asp agent session dispatch-claim --name {}",
                    dispatch_shell_argument(name)
                );
                if let Some(root_session_id) = root_session_id.as_deref() {
                    command.push_str(&format!(
                        " --root-session-id {}",
                        dispatch_shell_argument(root_session_id)
                    ));
                }
                command.push_str(&format!(
                    " --receipt-kind {} --command-json {} --resident-bridge --json",
                    dispatch_shell_argument(receipt_kind),
                    dispatch_shell_argument(&canonical_command_json),
                ));
                choice.platform_action = std::borrow::Cow::Owned(command);
                choice.required_inputs = &[];
            }
        }
        (None, None) => {}
        _ => {
            return Err(
                "bootstrap dispatch projection requires both --receipt-kind and --command-json"
                    .to_string(),
            );
        }
    }
    if args.json {
        let mut rendered = serde_json::to_value(&menu)
            .map_err(|error| format!("failed to render bootstrap menu: {error}"))?;
        if platform == "codex"
            && let Some(object) = rendered.as_object_mut()
        {
            let metadata = codex_spawn_agent_metadata_capability();
            object.insert(
                "hostTypedSpawnProjection".to_string(),
                serde_json::json!({
                    "schemaId": "agent.semantic-protocols.codex-typed-spawn-capability",
                    "schemaVersion": "1",
                    "spawnAgentMetadata": metadata,
                    "agentTypeProjected": metadata == "visible-agent-type",
                    "metadataHiddenByConfig": metadata == "hidden-by-config",
                    "diagnosticOnly": true,
                    "lifecycleAuthority": "live-host-spawn-schema-and-agent-tree-audit",
                    "fallbackAuthority": "explicit-choice-after-live-typed-transport-audit"
                }),
            );
            object.insert(
                "hostTypedSpawnObservation".to_string(),
                host_typed_spawn_observation.as_ref().map_or(
                    serde_json::Value::Null,
                    |observation| {
                        serde_json::json!({
                            "schemaId": observation.schema_id,
                            "schemaVersion": observation.schema_version,
                            "rootSessionId": observation.root_session_id,
                            "residentName": observation.resident_name,
                            "requiredField": observation.required_field,
                            "requiredValue": observation.required_value,
                            "fieldStatus": observation.field_status,
                            "source": observation.source,
                            "schemaDigest": observation.schema_digest,
                            "observedAt": observation.observed_at,
                            "expiresAt": observation.expires_at,
                            "fresh": true,
                            "diagnosticOnly": true,
                            "registersResidentChild": false,
                            "authorizesFallback": false,
                        })
                    },
                ),
            );
            object.insert(
                "hostResidentTargetObservation".to_string(),
                host_resident_target_observation.as_ref().map_or(
                    serde_json::Value::Null,
                    |observation| {
                        serde_json::json!({
                            "schemaId": observation.schema_id,
                            "schemaVersion": observation.schema_version,
                            "rootSessionId": observation.root_session_id,
                            "residentName": observation.resident_name,
                            "canonicalTarget": canonical_resident_target(&menu.host_requirement),
                            "targetStatus": observation.target_status,
                            "identityStatus": observation.identity_status,
                            "probeEvidenceRef": observation.probe_evidence_ref,
                            "source": observation.source,
                            "observedAt": observation.observed_at,
                            "expiresAt": observation.expires_at,
                            "fresh": true,
                            "registersResidentChild": host_resident_target_registers_child,
                        })
                    },
                ),
            );
            if let Some(observation) = host_resident_target_observation
                .as_ref()
                .filter(|observation| observation.target_status != "present")
            {
                binding::insert_non_present_canonical_target_receipt(
                    object,
                    record.as_ref(),
                    &observation.target_status,
                    host_typed_spawn_observation
                        .as_ref()
                        .map(|observation| observation.field_status.as_str()),
                    &canonical_resident_target(&menu.host_requirement),
                );
            }
            object.insert(
                "hostTypedSpawnClassification".to_string(),
                host_typed_spawn_observation.as_ref().map_or(
                    serde_json::json!({
                        "status": "audit-required",
                        "bootstrapBlocked": serde_json::Value::Null,
                        "fallbackAuthorized": false,
                    }),
                    |observation| {
                        serde_json::json!({
                            "status": if observation.field_status == "present" {
                                "typed-spawn-available"
                            } else {
                                "typed-spawn-unavailable"
                            },
                            "bootstrapBlocked": if observation.field_status == "absent" {
                                serde_json::Value::String(
                                    "host-typed-spawn-unavailable".to_string(),
                                )
                            } else {
                                serde_json::Value::Null
                            },
                            "fallbackAuthorized": false,
                            "nextAction": if observation.field_status == "present" {
                                "continue-host-tree-audit-before-typed-create"
                            } else {
                                "blocked-host-typed-spawn-unavailable"
                            },
                        })
                    },
                ),
            );
        }
        if profile_attestation_target_verified
            && let Some(observation) = runtime_drift.as_ref()
            && let Some(object) = rendered.as_object_mut()
        {
            let canonical_target = canonical_resident_target(&menu.host_requirement);
            let diagnosis = runtime_repair_diagnosis(&menu, observation);
            let switch_message = runtime_switch_followup_message(&menu);
            let natural_language = main_agent_runtime_rebind_instruction(
                &canonical_target,
                observation,
                menu.host_requirement.managed_agent_kind.as_ref(),
            );
            // Repeated observations do not make a nonexistent same-child
            // override become available. Keep offering one typed replacement;
            // only a missing host retire/spawn capability blocks that action.
            let repair_blocked = false;
            object.insert(
                "hostControlDirective".to_string(),
                serde_json::json!({
                    "schemaId": "agent.semantic-protocols.agent-session-host-control-directive",
                    "schemaVersion": "1",
                    "intent": if repair_blocked {
                        "report-runtime-override-unavailable"
                    } else {
                        "replace-drifted-resident-with-typed-role"
                    },
                    "target": canonical_target,
                    "childSessionId": observation.child_session_id,
                    "managedAgentKind": menu.host_requirement.managed_agent_kind.as_ref(),
                    "identityPolicy": "retire-before-replacement",
                    "createPolicy": "single-typed-replacement-only",
                    "instructionMode": "host-native-lifecycle",
                    "naturalLanguage": natural_language,
                    "mainAgentAction": if repair_blocked {
                        serde_json::Value::Null
                    } else {
                        serde_json::json!({
                            "surface": "host-native-retire-and-typed-spawn",
                            "arguments": {
                                "target": canonical_target,
                                "message": switch_message,
                            }
                        })
                    },
                    "desiredRuntime": {
                        "model": menu.expected_model,
                        "reasoningEffort": menu.expected_reasoning_effort,
                    },
                    "controlChannel": {
                            "requiredSurface": "host-native-retire-and-typed-spawn",
                            "requiredParameters": ["target", "agent_type", "task_name", "fork_turns"],
                            "runtimeApplication": "codex-registered-role-config",
                        "message": if repair_blocked {
                            serde_json::Value::Null
                        } else {
                            serde_json::Value::String(switch_message.clone())
                        },
                        "taskMessageCarriesControlIntent": false,
                        "taskMessageIsRuntimeEvidence": false,
                        "forbiddenFallbacks": [
                            "child-self-resume",
                            "controller-sibling",
                            "generic-replacement-child",
                            "codex-app-server-direct-input-to-multi-agent-v2-subagent"
                        ]
                    },
                    "verification": {
                        "source": "fresh-host-runtime-observation",
                        "requiredMatches": ["childSessionId", "managedAgentKind", "model", "reasoningEffort"]
                    },
                    "unavailable": {
                        "nextState": "Blocked",
                        "bootstrapBlocked": diagnosis.bootstrap_blocked,
                        "observedAfterSameChildResume": repair_blocked
                    }
                }),
            );
            object.insert(
                "hostLifecycleObservation".to_string(),
                serde_json::json!({
                    "status": "resident-child-runtime-drift",
                    "rootSessionId": observation.root_session_id,
                    "childSessionId": observation.child_session_id,
                    "observedAgentType": observation.observed_agent_type,
                    "expectedAgentType": observation.expected_agent_type,
                    "observedModel": observation.observed_model,
                    "observedReasoningEffort": observation.observed_reasoning_effort,
                    "driftDimensions": diagnosis.drift_dimensions,
                    "consecutiveObservationCount": observation.consecutive_observation_count,
                    "repairAttemptStatus": if repair_blocked {
                        diagnosis.repair_attempt_status
                    } else {
                        "typed-resident-replacement-required"
                    },
                    "expectedModel": expected_model,
                    "expectedReasoningEffort": expected_reasoning_effort,
                    "nextAction": if repair_blocked {
                        "report-host-typed-replacement-unavailable"
                    } else {
                        "retire-drifted-child-and-create-configured-replacement"
                    },
                    "runtimeOverrideOwner": "none-host-does-not-expose-same-child-switch",
                    "runtimeSwitchIntentInFollowupMessage": false,
                    "runtimeSwitchMessageIsEvidence": false,
                    "preserveChildIdentity": false,
                    "preserveManagedProfileIntent": true,
                    "managedProfileAttestation": if observation.observed_agent_type == observation.expected_agent_type {
                        "observed-agent-type-match"
                    } else {
                        "not-established-by-host-agent-type"
                    },
                }),
            );
        }
        if runtime_drift.is_none()
            && format!("{:?}", menu.state) == "Repair"
            && let Some(ref session) = menu.session
        {
            let canonical_target = canonical_resident_target(&menu.host_requirement);
            let switch_message = runtime_switch_followup_message(&menu);
            let managed_agent_kind = menu.host_requirement.managed_agent_kind.as_ref();
            let natural_language = format!(
                "Retire/archive drifted target {canonical_target} and child {}, wait for terminal status and path release, then create exactly one replacement with agent_type={managed_agent_kind}, task_name={managed_agent_kind}, and fork_turns=none. Codex must resolve the registered role TOML; do not send a natural-language model switch. Expected profile summary: `{switch_message}`",
                session.child_session_id,
            );
            let observed_model = rendered
                .pointer("/session/model")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let observed_reasoning_effort = rendered
                .pointer("/session/reasoningEffort")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if let Some(object) = rendered.as_object_mut() {
                object.insert(
                    "hostControlDirective".to_string(),
                    serde_json::json!({
                        "schemaId": "agent.semantic-protocols.agent-session-host-control-directive",
                        "schemaVersion": "1",
                        "intent": "replace-drifted-resident-with-typed-role",
                        "target": canonical_target,
                        "childSessionId": session.child_session_id,
                        "managedAgentKind": menu.host_requirement.managed_agent_kind.as_ref(),
                        "identityPolicy": "retire-before-replacement",
                        "createPolicy": "single-typed-replacement-only",
                        "instructionMode": "host-native-lifecycle",
                        "naturalLanguage": natural_language,
                        "mainAgentAction": {
                            "surface": "host-native-retire-and-typed-spawn",
                            "arguments": {
                                "target": canonical_target,
                                "message": switch_message,
                            }
                        },
                        "desiredRuntime": {
                            "model": menu.expected_model,
                            "reasoningEffort": menu.expected_reasoning_effort,
                        },
                        "controlChannel": {
                            "requiredSurface": "host-native-retire-and-typed-spawn",
                            "requiredParameters": ["target", "agent_type", "task_name", "fork_turns"],
                            "runtimeApplication": "codex-registered-role-config",
                            "message": switch_message,
                            "taskMessageCarriesControlIntent": false,
                            "taskMessageIsRuntimeEvidence": false,
                            "forbiddenFallbacks": [
                                "child-self-resume",
                                "controller-sibling",
                            "generic-replacement-child",
                                "codex-app-server-direct-input-to-multi-agent-v2-subagent"
                            ]
                        },
                        "verification": {
                            "source": "fresh-host-runtime-observation",
                            "requiredMatches": ["childSessionId", "managedAgentKind", "model", "reasoningEffort"]
                        }
                    }),
                );
                object.insert(
                    "hostLifecycleObservation".to_string(),
                    serde_json::json!({
                        "status": "resident-child-runtime-drift",
                        "rootSessionId": menu.root_session_id,
                        "childSessionId": session.child_session_id,
                        "observedModel": observed_model,
                        "observedReasoningEffort": observed_reasoning_effort,
                        "expectedModel": menu.expected_model,
                        "expectedReasoningEffort": menu.expected_reasoning_effort,
                        "nextAction": "retire-drifted-child-and-create-configured-replacement",
                        "observationSource": "codex-app-server-native-host-tree",
                        "runtimeSwitchMessageIsEvidence": false,
                        "preserveChildIdentity": false,
                    }),
                );
            }
        }
        if runtime_drift.is_none()
            && host_runtime_override_blocked
            && format!("{:?}", menu.state) == "Blocked"
            && let Some(ref session) = menu.session
        {
            let mut drift_dimensions = Vec::new();
            let mut blockers = Vec::new();
            if menu
                .expected_model
                .is_some_and(|expected| host_observed_model.as_deref() != Some(expected))
            {
                drift_dimensions.push("model");
                blockers.push("host-typed-resident-replacement-unavailable");
            }
            if menu
                .expected_reasoning_effort
                .is_some_and(|expected| host_observed_reasoning_effort.as_deref() != Some(expected))
            {
                drift_dimensions.push("reasoningEffort");
                blockers.push("host-typed-resident-replacement-unavailable");
            }
            if let Some(object) = rendered.as_object_mut() {
                object.insert(
                    "hostControlDirective".to_string(),
                    serde_json::json!({
                        "schemaId": "agent.semantic-protocols.agent-session-host-control-directive",
                        "schemaVersion": "1",
                        "intent": "report-typed-replacement-unavailable",
                        "target": canonical_resident_target(&menu.host_requirement),
                        "childSessionId": session.child_session_id,
                        "managedAgentKind": menu.host_requirement.managed_agent_kind.as_ref(),
                        "identityPolicy": "retire-before-replacement",
                        "createPolicy": "single-typed-replacement-only",
                        "mainAgentAction": serde_json::Value::Null,
                        "bootstrapBlocked": blockers,
                        "driftDimensions": drift_dimensions,
                        "verification": {
                            "source": "fresh-codex-app-server-host-tree-observation",
                            "freshSameChildObservation": fresh_same_child_runtime_observation,
                            "result": "typed-resident-replacement-unavailable"
                        }
                    }),
                );
                object.insert(
                    "hostLifecycleObservation".to_string(),
                    serde_json::json!({
                        "status": "resident-child-typed-replacement-blocked",
                        "rootSessionId": menu.root_session_id,
                        "childSessionId": session.child_session_id,
                        "sameChildIdentity": true,
                        "observedModel": host_observed_model,
                        "observedReasoningEffort": host_observed_reasoning_effort,
                        "expectedModel": menu.expected_model,
                        "expectedReasoningEffort": menu.expected_reasoning_effort,
                        "driftDimensions": drift_dimensions,
                        "freshSameChildObservation": fresh_same_child_runtime_observation,
                        "nextAction": "allow-unrelated-tools-and-report-host-typed-replacement-gap",
                        "preserveChildIdentity": false,
                        "replacementAllowed": true,
                    }),
                );
            }
        }
        if profile_attestation_target_verified
            && runtime_verification_evidence_incomplete
            && let Some(observation) = runtime_verification_observation.as_ref()
        {
            reasoning::insert_runtime_evidence_incomplete_receipt(
                &mut rendered,
                observation,
                &canonical_resident_target(&menu.host_requirement),
                menu.host_requirement.managed_agent_kind.as_ref(),
                expected_model.as_deref(),
                expected_reasoning_effort.as_deref(),
                runtime_reasoning_from_host,
                registry_routable,
            );
        }
        if profile_attestation_target_verified
            && let Some(observation) = runtime_verified.as_ref()
            && let Some(object) = rendered.as_object_mut()
        {
            let followup_ack_rebind =
                host_resident_target_observation
                    .as_ref()
                    .is_some_and(|observation| {
                        observation.source == "native-collaboration-followup-ack"
                            && observation.target_status == "present"
                            && observation.identity_status == "verified"
                    });
            object.insert(
                "hostControlDirective".to_string(),
                serde_json::json!({
                    "schemaId": "agent.semantic-protocols.agent-session-host-control-directive",
                    "schemaVersion": "1",
                    "intent": if followup_ack_rebind {
                        "same-child-followup-ack-rebind"
                    } else {
                        "typed-resident-replacement-verified"
                    },
                    "target": canonical_resident_target(&menu.host_requirement),
                    "childSessionId": observation.child_session_id,
                    "managedAgentKind": menu.host_requirement.managed_agent_kind.as_ref(),
                    "identityPolicy": if followup_ack_rebind {
                        "same-canonical-child-rebound-by-native-followup-ack"
                    } else {
                        "new-typed-child-replaces-drifted-owner"
                    },
                    "createPolicy": if followup_ack_rebind {
                        "not-created"
                    } else {
                        "completed"
                    },
                    "mainAgentAction": serde_json::Value::Null,
                    "verification": {
                        "source": observation.observation_source,
                        "result": reasoning::profile_attested_control_result(runtime_reasoning_profile_attested)
                    }
                }),
            );
            object.insert(
                "hostLifecycleObservation".to_string(),
                serde_json::json!({
                    "status": reasoning::profile_attested_lifecycle_status(runtime_reasoning_profile_attested),
                    "rootSessionId": observation.root_session_id,
                    "childSessionId": observation.child_session_id,
                    "sameChildIdentity": followup_ack_rebind,
                    "typedReplacementVerified": !followup_ack_rebind,
                    "verificationSource": observation.observation_source,
                    "observationCount": observation.observation_count,
                    "previousObservedModel": observation.previous_observed_model,
                    "previousObservedReasoningEffort": observation.previous_observed_reasoning_effort,
                    "observedModel": observation.observed_model,
                    "observedReasoningEffort": observation.observed_reasoning_effort,
                    "expectedModel": expected_model,
                    "expectedReasoningEffort": expected_reasoning_effort,
                    "modelMatchesExpected": expected_model.as_deref().is_some_and(|expected| observation.observed_model == expected),
                    "reasoningMatchesExpected": expected_reasoning_effort.as_deref().is_none_or(|expected| observation.observed_reasoning_effort.as_deref() == Some(expected)),
                    "reasoningRuntimeObserved": observation.observed_reasoning_effort.is_some(),
                    "reasoningVerificationStatus": if runtime_reasoning_profile_attested {
                        "host-profile-attested-unobservable"
                    } else {
                        "observed-match"
                    },
                    "reasoningEvidenceSource": if runtime_reasoning_profile_attested {
                        reasoning::profile_attestation_evidence_source(observation.observation_source)
                    } else if runtime_reasoning_from_host {
                        "codex-app-server-thread-resume"
                    } else {
                        observation.observation_source
                    },
                    "profileAttestation": reasoning::profile_attestation_receipt(
                        runtime_reasoning_profile_attested,
                        observation,
                        menu.host_requirement.managed_agent_kind.as_ref(),
                        (attested_child_id.as_ref(), expected_reasoning_effort.as_ref()),
                    ),
                    "registryRoutable": registry_routable,
                    "nextAction": if registry_routable {
                "dispatch-resident-command"
                    } else {
                        "rehydrate-verified-existing-child-registry"
                    }
                }),
            );
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&rendered)
                .map_err(|error| format!("failed to render bootstrap menu: {error}"))?
        );
    } else {
        render::print_bootstrap_menu(&menu, runtime_drift.as_ref(), runtime_verified.as_ref());
        if let Some(observation) = host_typed_spawn_observation.as_ref() {
            println!(
                "host-typed-spawn-observation: field=agent_type status={} source={} expiresAt={} diagnosticOnly=true registersResidentChild=false authorizesFallback=false",
                observation.field_status, observation.source, observation.expires_at,
            );
            if observation.field_status == "absent" {
                println!(
                    "host-typed-spawn-classification: status=typed-spawn-unavailable bootstrapBlocked=host-typed-spawn-unavailable fallbackAuthorized=false nextAction=blocked-host-typed-spawn-unavailable"
                );
            } else {
                println!(
                    "host-typed-spawn-classification: status=typed-spawn-available fallbackAuthorized=false nextAction=continue-host-tree-audit-before-typed-create"
                );
            }
        } else {
            println!(
                "host-typed-spawn-observation: missing-or-stale diagnosticOnly=true registersResidentChild=false authorizesFallback=false"
            );
        }
        if let Some(observation) = host_resident_target_observation.as_ref() {
            println!(
                "host-resident-target-observation: target={} status={} identityStatus={} source={} expiresAt={} fresh=true registersResidentChild={}",
                canonical_resident_target(&menu.host_requirement),
                observation.target_status,
                observation.identity_status,
                observation.source,
                observation.expires_at,
                host_resident_target_registers_child,
            );
        } else {
            println!(
                "host-resident-target-observation: missing-or-stale target={} registersResidentChild=false",
                canonical_resident_target(&menu.host_requirement),
            );
        }
    }
    Ok(())
}
