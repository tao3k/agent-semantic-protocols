//! Project-wide topology identity and admission boundary.
#![forbid(unsafe_code)]

mod project_topology_library;
mod project_topology_program_binding;

pub use project_topology_library::{
    PROJECT_TOPOLOGY_LIBRARY_SCHEMA_ID, PROJECT_TOPOLOGY_LIBRARY_SCHEMA_VERSION,
    ProjectTopologyLibrary, ProjectTopologyLibraryError,
};
pub use project_topology_program_binding::{
    ProjectTopologyProgramBindingError, admit_project_topology_program_bundle,
};
