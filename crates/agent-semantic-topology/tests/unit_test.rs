// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
#[path = "unit/project_topology_program_binding.rs"]
mod project_topology_program_binding;
#[path = "unit/runtime_project_topology_attachment.rs"]
mod runtime_project_topology_attachment;
#[path = "unit/support.rs"]
mod support;
#[path = "unit/topology_index.rs"]
mod topology_index;

pub(crate) use support::canonical_test_manifest;
