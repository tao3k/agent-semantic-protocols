// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[cfg(feature = "embedded-orgize")]
#[path = "unit/agent_org_topology_overlay.rs"]
mod agent_org_topology_overlay;
#[path = "unit/org_projection_port.rs"]
mod org_projection_port;
#[path = "unit/project_topology_closure.rs"]
mod project_topology_closure;
#[path = "unit/project_topology_frontier.rs"]
mod project_topology_frontier;
#[path = "unit/project_topology_generation_builder.rs"]
mod project_topology_generation_builder;
#[path = "unit/project_topology_library.rs"]
mod project_topology_library;
#[cfg(feature = "embedded-orgize")]
#[path = "unit/project_topology_manifest.rs"]
mod project_topology_manifest;
#[path = "unit/project_topology_program_binding.rs"]
mod project_topology_program_binding;
#[cfg(feature = "embedded-orgize")]
#[path = "unit/project_topology_source_program.rs"]
mod project_topology_source_program;
#[path = "unit/runtime_project_topology_attachment.rs"]
mod runtime_project_topology_attachment;
#[path = "unit/support.rs"]
mod support;
#[path = "unit/topology_index.rs"]
mod topology_index;

pub(crate) use support::canonical_test_manifest;
#[cfg(feature = "embedded-orgize")]
pub(crate) use support::ensure_orgize_native_runtime;
