use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::agent_session_registry::{
    AgentSessionInteractiveMenu, AgentSessionLookupRequest, AgentSessionRegistry,
    ResidentChildBootstrapMenuInput, resident_child_bootstrap_menu,
    resident_child_host_runtime_refresh_eligible, resident_child_runtime_repair_menu,
    resident_child_runtime_verified_menu,
};
use agent_semantic_client_db::{AgentSessionRecord, agent_session_message_target_is_live_bound};

use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_state::{project_session_scope_id, resolved_root_session_id};

use agent_semantic_client_db::agent_session_registry::AgentSessionHostRequirement;

pub(super) fn bootstrap_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    project_root: &Path,
) -> Result<(), String> {
    let project_id = project_session_scope_id(registry, project_root)?;
    let name = args.name.as_deref().unwrap_or("asp-explore");
    reject_resident_child_bootstrap(registry, &project_id, name)?;
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let now = unix_timestamp()?;
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
    let mut registry_routable = record
        .as_ref()
        .is_some_and(|session| registry_record_routable(session, root_session_id.as_deref(), now));
    if !registry_routable {
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
            registry_record_routable(session, root_session_id.as_deref(), now)
        });
    }
    if host_resident_target_observation
        .as_ref()
        .is_some_and(|observation| observation.target_status == "absent")
        && let Some(existing) = record.as_mut()
        && !matches!(
            existing.status.as_str(),
            "archived" | "closed" | "orphan-risk"
        )
    {
        registry.update_session_status(&project_id, &existing.session_id, "orphan-risk", now)?;
        existing.status = "orphan-risk".to_string();
        registry_routable = false;
    }
    if host_resident_target_observation
        .as_ref()
        .is_some_and(|observation| observation.target_status == "present")
        && let Some(existing) = record.as_ref()
        && let Some(root_session_id) = root_session_id.as_deref()
        && (!agent_semantic_client_db::agent_session_registry::agent_session_message_target_is_live_bound(
            existing,
            root_session_id,
        ) || existing.message_target_id.as_deref() != Some("/root/asp_explorer")
            || !existing.is_routable_at(now))
    {
        // The registry `role` is the ASP semantic permission role (for
        // example `search,subagent`), not the Codex managed agent kind.
        // The native v2 target path is owned by managedAgentKind.
        let message_target_id = "/root/asp_explorer".to_string();
        let mut metadata = serde_json::from_str::<serde_json::Value>(&existing.metadata_json)
            .unwrap_or_else(|_| serde_json::json!({}));
        if !metadata.is_object() {
            metadata = serde_json::json!({});
        }
        metadata["messageTargetBinding"] = serde_json::json!({
            "source": "native-collaboration-list-agents",
            "boundRootSessionId": root_session_id,
            "childSessionId": existing.session_id,
            "messageTargetId": message_target_id,
            "observedAt": now,
        });
        let model_observation = match (
            existing.model.as_deref(),
            existing.model_observation_source.as_deref(),
            existing.model_observed_at,
        ) {
            (Some(model), Some("codex.subagent-start"), Some(observed_at)) => Some(
                agent_semantic_client_db::agent_session_registry::AgentSessionModelObservationRef {
                    model,
                    source: agent_semantic_client_db::agent_session_registry::AgentSessionModelObservationSource::CodexSubagentStart,
                    observed_at,
                    evidence_ref: existing.model_evidence_ref.as_deref(),
                },
            ),
            (Some(model), Some("codex.rollout"), Some(observed_at)) => Some(
                agent_semantic_client_db::agent_session_registry::AgentSessionModelObservationRef {
                    model,
                    source: agent_semantic_client_db::agent_session_registry::AgentSessionModelObservationSource::CodexRollout,
                    observed_at,
                    evidence_ref: existing.model_evidence_ref.as_deref(),
                },
            ),
            _ => None,
        };
        registry.archive_session(&project_id, &existing.session_id, now)?;
        let rebound = registry.claim_resident_session(
            agent_semantic_client_db::agent_session_registry::AgentSessionRegisterRequest {
                project_id: &project_id,
                root_session_id,
                session_id: &existing.session_id,
                message_target_id: Some(&message_target_id),
                parent_session_id: Some(root_session_id),
                name,
                role: &existing.role,
                model_observation,
                // Registry routability is expressed as active|idle; `Ready`
                // is the derived bootstrap state, not a persisted row status.
                status: "idle",
                // A fresh native host-tree presence observation renews the
                // resident route. Never carry an expired transient lease into
                // the repaired durable resident binding.
                expires_at: None,
                metadata_json: &metadata.to_string(),
                now,
            },
        )?;
        record = Some(rebound);
        registry_routable = true;
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
    let mut fresh_same_child_runtime_observation = false;
    let mut host_runtime_override_blocked = false;
    let mut host_observed_model = None;
    let mut host_observed_reasoning_effort = None;
    if platform == "codex"
        && let (Some(root_session_id), Some(existing)) =
            (root_session_id.as_deref(), record.as_ref())
        && resident_child_host_runtime_refresh_eligible(
            registry_routable,
            existing,
            root_session_id,
        )
    {
        let refresh = super::agent_session_registry_profile::refresh_existing_codex_host_runtime(
            registry,
            &project_id,
            root_session_id,
            existing,
            expected_model.as_deref(),
            expected_reasoning_effort.as_deref(),
            now,
        )?;
        fresh_same_child_runtime_observation = refresh.fresh_after_previous_observation;
        host_runtime_override_blocked = refresh.runtime_override_blocked;
        host_observed_model = refresh.observed_model;
        host_observed_reasoning_effort = refresh.observed_reasoning_effort;
        if let Some(refreshed) = refresh.record {
            record = Some(refreshed);
            registry_routable = record.as_ref().is_some_and(|session| {
                registry_record_routable(session, Some(root_session_id), now)
            });
        }
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
        } else {
            menu.choices
                .retain(|choice| choice.id != "activate-inline-parser-fallback");
        }
    }
    let runtime_drift = root_session_id
        .as_deref()
        .map(|root_session_id| {
            agent_semantic_hook::latest_subagent_runtime_drift(project_root, root_session_id)
        })
        .transpose()?
        .flatten()
        .filter(|observation| {
            record
                .as_ref()
                .is_none_or(|session| session.session_id == observation.child_session_id)
        });
    let runtime_verified = root_session_id
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
            record
                .as_ref()
                .is_none_or(|session| session.session_id == observation.child_session_id)
        });
    if registry_routable && runtime_verified.is_some() {
        menu = resident_child_runtime_verified_menu(menu, registry_routable);
    } else if let Some(observation) = runtime_drift.as_ref() {
        menu = resident_child_runtime_repair_menu(menu, observation.consecutive_observation_count);
    } else if registry_routable && host_runtime_override_blocked {
        menu = resident_child_runtime_repair_menu(menu, 2);
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
                            "source": observation.source,
                            "observedAt": observation.observed_at,
                            "expiresAt": observation.expires_at,
                            "fresh": true,
                            "registersResidentChild": false,
                        })
                    },
                ),
            );
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
                                serde_json::Value::String("host-agent-type-unavailable".to_string())
                            } else {
                                serde_json::Value::Null
                            },
                            "fallbackAuthorized": false,
                            "nextAction": if observation.field_status == "present" {
                                "continue-host-tree-audit-before-typed-create"
                            } else {
                                "choose-explicit-inline-parser-fallback-for-exact-denied-command"
                            },
                        })
                    },
                ),
            );
        }
        if let Some(observation) = runtime_drift.as_ref()
            && let Some(object) = rendered.as_object_mut()
        {
            let canonical_target = canonical_resident_target(&menu.host_requirement);
            let diagnosis = runtime_repair_diagnosis(&menu, observation);
            let switch_message = runtime_switch_followup_message(&menu);
            let natural_language =
                main_agent_runtime_rebind_instruction(&canonical_target, observation);
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
                    "managedAgentKind": menu.host_requirement.managed_agent_kind,
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
            let natural_language = format!(
                "Retire/archive drifted target {canonical_target} and child {}, wait for terminal status and path release, then create exactly one replacement with agent_type=asp_explorer, task_name=asp_explorer, and fork_turns=none. Codex must resolve the registered role TOML; do not send a natural-language model switch. Expected profile summary: `{switch_message}`",
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
                        "managedAgentKind": menu.host_requirement.managed_agent_kind,
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
                        "managedAgentKind": menu.host_requirement.managed_agent_kind,
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
        if let Some(observation) = runtime_verified.as_ref()
            && let Some(object) = rendered.as_object_mut()
        {
            object.insert(
                "hostControlDirective".to_string(),
                serde_json::json!({
                    "schemaId": "agent.semantic-protocols.agent-session-host-control-directive",
                    "schemaVersion": "1",
                    "intent": "typed-resident-replacement-verified",
                    "target": canonical_resident_target(&menu.host_requirement),
                    "childSessionId": observation.child_session_id,
                    "managedAgentKind": menu.host_requirement.managed_agent_kind,
                    "identityPolicy": "new-typed-child-replaces-drifted-owner",
                    "createPolicy": "completed",
                    "mainAgentAction": serde_json::Value::Null,
                    "verification": {
                        "source": observation.observation_source,
                        "result": "observed-runtime-matches-expected"
                    }
                }),
            );
            object.insert(
                "hostLifecycleObservation".to_string(),
                serde_json::json!({
                    "status": "resident-child-typed-replacement-verified",
                    "rootSessionId": observation.root_session_id,
                    "childSessionId": observation.child_session_id,
                    "sameChildIdentity": false,
                    "typedReplacementVerified": true,
                    "verificationSource": observation.observation_source,
                    "observationCount": observation.observation_count,
                    "previousObservedModel": observation.previous_observed_model,
                    "previousObservedReasoningEffort": observation.previous_observed_reasoning_effort,
                    "observedModel": observation.observed_model,
                    "observedReasoningEffort": observation.observed_reasoning_effort,
                    "expectedModel": observation.expected_model,
                    "expectedReasoningEffort": observation.expected_reasoning_effort,
                    "modelMatchesExpected": observation.observed_model == observation.expected_model,
                    "reasoningMatchesExpected": observation.observed_reasoning_effort == observation.expected_reasoning_effort,
                    "registryRoutable": registry_routable,
                    "nextAction": if registry_routable {
                        "send-denied-asp-command"
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
        print_bootstrap_menu(&menu, runtime_drift.as_ref(), runtime_verified.as_ref());
        if let Some(observation) = host_typed_spawn_observation.as_ref() {
            println!(
                "host-typed-spawn-observation: field=agent_type status={} source={} expiresAt={} diagnosticOnly=true registersResidentChild=false authorizesFallback=false",
                observation.field_status, observation.source, observation.expires_at,
            );
            if observation.field_status == "absent" {
                println!(
                    "host-typed-spawn-classification: status=typed-spawn-unavailable bootstrapBlocked=host-agent-type-unavailable fallbackAuthorized=false nextAction=choose-explicit-inline-parser-fallback-for-exact-denied-command"
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
                "host-resident-target-observation: target={} status={} source={} expiresAt={} fresh=true registersResidentChild=false",
                canonical_resident_target(&menu.host_requirement),
                observation.target_status,
                observation.source,
                observation.expires_at,
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

fn reject_resident_child_bootstrap(
    _registry: &AgentSessionRegistry,
    _project_id: &str,
    name: &str,
) -> Result<(), String> {
    let project_root = std::env::current_dir()
        .map_err(|error| format!("failed to read current directory: {error}"))?;
    let runtime_session = agent_semantic_runtime::current_agent_runtime_session();
    let non_root_session = runtime_session.as_ref().is_some_and(|session| {
        super::agent_session_registry_state::current_root_session_id()
            .is_some_and(|root_session_id| root_session_id != session.id)
    });
    if non_root_session
        || super::agent_session_registry_state::current_resident_child_identity_proof(
            &project_root,
            name,
            "",
        )?
        .is_some()
    {
        let session_id = runtime_session
            .map(|session| session.id)
            .unwrap_or_else(|| "<unknown>".to_string());
        return Err(format!(
            "bootstrap-owner-main-session-only: registered resident child session `{session_id}` must use parser-owned ASP query/search directly and return its receipt; do not enter or execute the lifecycle bootstrap from the child."
        ));
    }
    Ok(())
}

fn print_bootstrap_menu(
    menu: &AgentSessionInteractiveMenu<'_>,
    runtime_drift: Option<&agent_semantic_hook::SubagentRuntimeDriftObservation>,
    runtime_verified: Option<&agent_semantic_hook::SubagentRuntimeRebindVerifiedObservation>,
) {
    let state = format!("{:?}", menu.state);
    println!(
        "pane: asp.session.{}.v{}",
        state.to_ascii_lowercase(),
        menu.schema_version
    );
    println!("state: {state}");
    println!("target: {}", menu.name);
    if let Some(root_session_id) = menu.root_session_id {
        println!("root-session: {root_session_id}");
    }
    let why = runtime_verified.map_or_else(
        || {
            runtime_drift.map_or_else(
                || latest_trace_result(menu),
                |_| "typed-resident-replacement-required".to_string(),
            )
        },
        |_| "typed-resident-replacement-verified".to_string(),
    );
    println!("why: {why}");
    if let Some(observation) = runtime_verified {
        println!(
            "typed-replacement-verification: verified=true child={} sameChildIdentity=false source={} observationCount={} previousModel={} observedModel={} expectedModel={} previousReasoning={} observedReasoning={} expectedReasoning={} modelMatchesExpected=true reasoningMatchesExpected=true registryRoutable={}",
            observation.child_session_id,
            observation.observation_source,
            observation.observation_count,
            observation
                .previous_observed_model
                .as_deref()
                .unwrap_or("unknown"),
            observation.observed_model,
            observation.expected_model,
            observation
                .previous_observed_reasoning_effort
                .as_deref()
                .unwrap_or("unknown"),
            observation
                .observed_reasoning_effort
                .as_deref()
                .unwrap_or("unknown"),
            observation
                .expected_reasoning_effort
                .as_deref()
                .unwrap_or("unknown"),
            menu.session.is_some(),
        );
    }
    if let Some(observation) = runtime_drift {
        let diagnosis = runtime_repair_diagnosis(menu, observation);
        println!(
            "host-lifecycle: child={} expectedAgentType={} observedAgentType={} expectedModel={} observedModel={} expectedReasoning={} observedReasoning={} driftDimensions={} consecutiveObservationCount={} repairAttemptStatus=typed-resident-replacement-required action=retire-drifted-child-and-create-configured-replacement runtimeOverrideOwner=none runtimeSwitchIntentInFollowupMessage=false preserveChildIdentity=false managedProfileAttestation={}",
            observation.child_session_id,
            observation.expected_agent_type,
            observation.observed_agent_type,
            menu.expected_model.unwrap_or("unknown"),
            observation.observed_model.as_deref().unwrap_or("unknown"),
            menu.expected_reasoning_effort.unwrap_or("unknown"),
            observation
                .observed_reasoning_effort
                .as_deref()
                .unwrap_or("unknown"),
            diagnosis.drift_dimensions.join(","),
            observation.consecutive_observation_count,
            if observation.observed_agent_type == observation.expected_agent_type {
                "observed-agent-type-match"
            } else {
                "not-established-by-host-agent-type"
            },
        );
        println!(
            "repair: retire/archive the drifted child, wait for terminal host status and canonical-path release, then create exactly one replacement with agent_type={}, task_name={}, and fork_turns=none. Codex must load the registered TOML; do not send a natural-language model switch.",
            menu.host_requirement.managed_agent_kind, menu.host_requirement.managed_agent_kind,
        );
        let canonical_target = canonical_resident_target(&menu.host_requirement);
        println!(
            "main-agent-control-directive: {}",
            main_agent_runtime_rebind_instruction(&canonical_target, observation)
        );
        println!(
            "host-control-contract: target={} identityPolicy=retire-before-replacement createPolicy=single-typed-replacement-only instructionMode=host-native-lifecycle requiredSurface=host-native-retire-and-typed-spawn requiredParameters=target,agent_type,task_name,fork_turns runtimeApplication=codex-registered-role-config taskMessageCarriesControlIntent=false verification=typed-SubagentStart",
            canonical_target
        );
        println!(
            "host-control-blocker: nextState=Blocked bootstrapBlocked={} driftDimensions={} policy=ASP-routing-degraded-but-unrelated-Codex-tools-allowed",
            diagnosis.bootstrap_blocked,
            diagnosis.drift_dimensions.join(","),
        );
    }
    println!(
        "must: prefer a profile-valid resident child; when typed spawn is unavailable, use only the explicit inline parser fallback choice; never substitute raw source access"
    );
    println!(
        "transport: preferred=native.{}.{} fallback=current-session-parser; resident lifecycle identity is captured by host hooks",
        menu.host_requirement.platform, menu.host_requirement.required_transport
    );
    if menu.host_requirement.platform == "codex" {
        println!(
            "profile-discovery: Codex auto-loads custom-agent TOML from ~/.codex/agents/ and .codex/agents/; bootstrap does not write [agents.<name>] registration"
        );
        let metadata = codex_spawn_agent_metadata_capability();
        println!(
            "host-typed-spawn-projection: spawnAgentMetadata={} agentTypeProjected={} metadataHiddenByConfig={} diagnosticOnly=true lifecycleAuthority=live-host-spawn-schema-and-agent-tree-audit fallbackAuthority=explicit-choice-after-live-typed-transport-audit",
            metadata,
            metadata == "visible-agent-type",
            metadata == "hidden-by-config",
        );
    }
    println!(
        "resident-registration: SubagentStart event owns registration and validation of the spawned runtime child identity"
    );
    let has_create_choice = menu
        .choices
        .iter()
        .any(|choice| choice.id == "create-managed-resident-child-after-host-tree-miss");
    if has_create_choice {
        println!(
            "host-typed-spawn-preflight: tool=collaboration.spawn_agent requiredField=agent_type requiredValue={} genericFieldsInsufficient=task_name,message,fork_turns unavailable=bootstrapBlocked=host-agent-type-unavailable",
            menu.host_requirement.managed_agent_kind,
        );
        println!(
            "platform-native-create: platform={} managedAgentKind={} action={}",
            menu.host_requirement.platform,
            menu.host_requirement.managed_agent_kind,
            platform_native_create_action(&menu.host_requirement)
        );
        println!(
            "platform-native-create-blocker: Create is authorized only after rollout and host-agent-tree audits both miss; if platform={} cannot audit or create managedAgentKind={} or emits no SubagentStart event, report the matching host lifecycle gap; do not create fallback agents or normal threads",
            menu.host_requirement.platform, menu.host_requirement.managed_agent_kind
        );
    }
    let has_inline_fallback_choice = menu
        .choices
        .iter()
        .any(|choice| choice.id == "activate-inline-parser-fallback");
    if has_inline_fallback_choice {
        println!(
            "inline-parser-fallback: available=true state=ReadyDegraded transport=current-session optIn=ASP_INLINE_PARSER_FALLBACK=1 policy=exact-parser-owned-command-only residentChild=false rawSourceFallback=false"
        );
    }
    if let Some(expected_model) = menu.expected_model {
        println!("model: expected {expected_model}");
    }
    if let Some(expected_reasoning_effort) = menu.expected_reasoning_effort {
        println!("reasoning: expected {expected_reasoning_effort}");
    }
    if let Some(status) = menu.rollout_history_status {
        println!(
            "rollout: status={} action={}",
            status,
            menu.rollout_history_action.unwrap_or("none")
        );
    }
    if let Some(session) = menu.session.as_ref() {
        println!(
            "session: child={} status={} role={} model={} messageTarget={}",
            session.child_session_id,
            session.status,
            session.role,
            session.model.unwrap_or("unknown"),
            session.message_target_status
        );
        if let Some(source) = session.model_observation_source {
            println!(
                "model-observation: source={} observedAt={} evidence={}",
                source,
                session.model_observed_at.unwrap_or_default(),
                session.model_evidence_ref.unwrap_or("none")
            );
        }
    } else {
        println!(
            "registry: missing; this does not mean the resident child is absent; audit rollout history and the native host agent tree before Create"
        );
    }
    println!();
    for (index, choice) in menu.choices.iter().enumerate() {
        println!("{}: {}", index + 1, choice.id);
        println!("  ask: {}", choice.label);
        println!("  do: {}", choice.platform_action);
        println!(
            "  expect: {}",
            required_inputs_phrase(choice.required_inputs)
        );
        println!("  after: {:?}", choice.next_state);
        println!();
    }
    println!(
        "select: return exactly one number, such as \"1\"; perform its do action through the declared transport; return expect; re-enter the loop at after."
    );
    println!(
        "lifecycle: after native create, typed replacement, or interrupt, re-enter this pane; never pass child ids, message targets, model claims, or lifecycle status as bootstrap command flags."
    );
}

fn codex_spawn_agent_metadata_capability() -> &'static str {
    let config_path = crate::command::sync::codex_home().join("config.toml");
    let Ok(content) = std::fs::read_to_string(config_path) else {
        return "unknown";
    };
    let Ok(config) = toml::from_str::<toml::Value>(&content) else {
        return "unknown";
    };
    match config
        .get("features")
        .and_then(|features| features.get("multi_agent_v2"))
        .and_then(|multi_agent_v2| multi_agent_v2.get("hide_spawn_agent_metadata"))
        .and_then(toml::Value::as_bool)
    {
        Some(true) => "hidden-by-config",
        Some(false) => "visible-agent-type",
        None => "unknown",
    }
}

fn platform_native_create_action(requirement: &AgentSessionHostRequirement<'_>) -> String {
    match requirement.platform {
        "codex" => format!(
            "use a native collaboration spawn surface that explicitly exposes `agent_type`; set `agent_type={}`, `task_name={}`, and `fork_turns=none`, then let Codex apply the auto-loaded TOML profile and let SubagentStart capture the native identity. `agent_type` selects the registered role; `task_name` reserves the canonical /root/{} path; natural-language task payload does not select the role. If `agent_type` is unavailable, report `host-agent-type-unavailable` without spawning a generic child",
            requirement.managed_agent_kind,
            requirement.managed_agent_kind,
            requirement.managed_agent_kind,
        ),
        _ => format!(
            "use the detected platform's managed-agent creation action for {}; let the platform lifecycle-start event capture native identity",
            requirement.managed_agent_kind
        ),
    }
}

fn canonical_resident_target(requirement: &AgentSessionHostRequirement<'_>) -> String {
    if requirement.platform == "codex" {
        format!("/root/{}", requirement.managed_agent_kind)
    } else {
        requirement.resident_child_name.to_string()
    }
}

fn registry_record_routable(
    record: &AgentSessionRecord,
    current_root_session_id: Option<&str>,
    now: i64,
) -> bool {
    !matches!(record.status.as_str(), "archived" | "closed")
        && current_root_session_id
            .is_some_and(|root| agent_session_message_target_is_live_bound(record, root))
        && record.is_routable_at(now)
}

fn main_agent_runtime_rebind_instruction(
    canonical_target: &str,
    observation: &agent_semantic_hook::SubagentRuntimeDriftObservation,
) -> String {
    format!(
        "Retire/archive drifted target {canonical_target} and child {}, wait for terminal host status and canonical-path release, then create exactly one replacement through a spawn surface that exposes agent_type, with agent_type=asp_explorer and fork_turns=none. task_name only names the tree node and message/natural-language task text is only payload; neither selects the registered role. Codex must load the complete registered TOML profile; if agent_type is unavailable, report the host capability blocker instead of spawning.",
        observation.child_session_id,
    )
}

fn runtime_switch_followup_message(menu: &AgentSessionInteractiveMenu<'_>) -> String {
    format!(
        "Retire the drifted resident and create one typed asp_explorer replacement from the registered Codex role. The expected runtime is model {} with reasoning {}, but Codex must obtain both values from the role TOML rather than this message.",
        menu.expected_model.unwrap_or("unknown"),
        menu.expected_reasoning_effort.unwrap_or("unknown"),
    )
}

struct RuntimeRepairDiagnosis {
    drift_dimensions: Vec<&'static str>,
    bootstrap_blocked: &'static str,
    repair_attempt_status: &'static str,
}

fn runtime_repair_diagnosis(
    menu: &AgentSessionInteractiveMenu<'_>,
    observation: &agent_semantic_hook::SubagentRuntimeDriftObservation,
) -> RuntimeRepairDiagnosis {
    let model_drift = menu
        .expected_model
        .is_some_and(|expected| observation.observed_model.as_deref() != Some(expected));
    let reasoning_drift = menu
        .expected_reasoning_effort
        .is_some_and(|expected| observation.observed_reasoning_effort.as_deref() != Some(expected));
    match (model_drift, reasoning_drift) {
        (false, true) => RuntimeRepairDiagnosis {
            drift_dimensions: vec!["reasoningEffort"],
            bootstrap_blocked: "host-typed-resident-replacement-unavailable",
            repair_attempt_status: "typed-resident-replacement-required",
        },
        (true, false) => RuntimeRepairDiagnosis {
            drift_dimensions: vec!["model"],
            bootstrap_blocked: "host-typed-resident-replacement-unavailable",
            repair_attempt_status: "typed-resident-replacement-required",
        },
        _ => RuntimeRepairDiagnosis {
            drift_dimensions: vec!["model", "reasoningEffort"],
            bootstrap_blocked: "host-typed-resident-replacement-unavailable",
            repair_attempt_status: "typed-resident-replacement-required",
        },
    }
}

fn latest_trace_result(menu: &AgentSessionInteractiveMenu<'_>) -> String {
    menu.trace
        .last()
        .map(|step| step.result)
        .or(menu.rollout_history_status)
        .unwrap_or("loop state requires action")
        .to_string()
}

fn required_inputs_phrase(values: &[&str]) -> String {
    if values.is_empty() {
        "native action observation".to_string()
    } else {
        values.join(",")
    }
}

fn unix_timestamp() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|error| format!("system clock before unix epoch: {error}"))
}
