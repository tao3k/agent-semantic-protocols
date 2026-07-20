//! Turso EvidenceGraph adapter.

mod artifact;

pub use artifact::graph_artifact_digest_for_snapshot;
mod edge_read;
mod entity_read;
mod model;
mod owner_read;
mod persist;

pub use artifact::{TURSO_EDGE_TABLE, TURSO_ENTITY_TABLE};
pub use edge_read::list_turso_graph_edges;
pub use entity_read::list_turso_graph_entities;
pub use model::{
    TursoClientDbEvidenceGraphPersistReport, TursoClientDbGraphEdge, TursoClientDbGraphEntity,
    TursoClientDbGraphOwnerReadModel,
};
pub use owner_read::lookup_turso_graph_owner_read_model;
pub use persist::persist_turso_evidence_graph;
