// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Hook client configuration interface.

mod document;
mod policy_coverage;
mod profiles;
mod routing;
mod service;
mod validation;

pub use validation::validate_codex_host_matcher_expression;

pub use routing::HookClientStructuredFilterGrammar;
pub use routing::HookClientStructuredFormat;
pub use routing::HookClientStructuredProjectionMatchConfig;

pub use document::AspProjectConfigFile;
pub use document::AspProjectDiscoveryConfig;
pub use document::AspProjectHookConfig;
pub use document::CLIENT_HOOK_CONFIG_SCHEMA_ID;
pub use document::CLIENT_HOOK_CONFIG_SCHEMA_VERSION;
pub use document::HookClientAgentCallingConfig;
pub use document::HookClientAgentOrgArtifactsArchiveWarningConfig;
pub use document::HookClientAgentOrgArtifactsConfig;
pub use document::HookClientCommandActionPatternConfig;
pub use document::HookClientConfigFile;
pub use document::HookClientProfileConfig;
pub use document::HookClientProviderRouteIdentity;
pub use document::WrapperMatchMode;
pub use document::default_hook_client_config_file;
pub use document::default_hook_client_config_template;
pub use document::hook_client_contract_fingerprint;
pub use document::load_asp_project_config_file;
pub use document::load_hook_client_config_declared_contract_fingerprint;
pub use document::render_hook_client_message_template;
pub use policy_coverage::HookPolicyCoverageCase;
pub use policy_coverage::HookPolicyCoverageLanguageId;
pub use policy_coverage::HookPolicyCoveragePath;
pub use policy_coverage::HookPolicyCoveragePolarity;
pub use policy_coverage::HookPolicyCoverageProviderId;
pub use policy_coverage::HookPolicyCoverageRuleId;
pub use policy_coverage::HookPolicyCoverageSettings;
pub use policy_coverage::HookPolicyCoverageSurface;
pub use policy_coverage::derive_hook_policy_coverage_cases;
pub use policy_coverage::mutate_path_outside_registered_extensions;
pub use profiles::HookClientCommandCategory;
pub use profiles::HookClientCommandProfileConfig;
pub use profiles::HookClientCommandProfileId;
pub use profiles::HookClientCommandProfileRef;
pub use profiles::HookClientCommandSetConfig;
pub use profiles::expand_command_profile_prefixes;
pub use profiles::expand_command_set_prefixes;
pub use routing::HookClientActionKind;
pub use routing::HookClientActionSubjectKind;
pub use routing::HookClientAgentSelector;
pub use routing::HookClientCapabilityPolicyConfig;
pub use routing::HookClientConfigDecision;
pub use routing::HookClientConfigReasonKind;
pub use routing::HookClientConfigRouteKind;
pub use routing::HookClientConfigStdinMode;
pub use routing::HookClientHostInvocationKind;
pub use routing::HookClientLazyProviderPolicy;
pub use routing::HookClientMatcherPolicy;
pub use routing::HookClientRuleConfig;
pub use routing::HookClientRuleDispatchConfig;
pub use routing::HookClientRuleDispatchTransport;
pub use routing::HookClientRuleMatchConfig;
pub use routing::HookClientRuleRouteConfig;
pub use service::load_hook_client_config_file;
pub use service::load_hook_client_config_overlay_file;
pub use service::merge_asp_project_hook_config;
