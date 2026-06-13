#![deny(dead_code)]

//! SQLite-backed cache database surface for `agent-semantic-client`.

pub mod db;
pub mod pragmas;
mod structural_index;
mod syntax_query;

pub use agent_semantic_client_core::ClientDbStatus;
pub use db::{
    AGENT_SEMANTIC_CLIENT_DB_FILE, AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION, ClientDb,
    ClientDbArtifactEvent, ClientDbGenerationHit, ClientDbGenerationLookup,
    ClientDbProviderCommandSelection, ClientDbReport, ClientDbSummary, ClientDbSyntaxCaptureReplay,
    ClientDbSyntaxNodeType, ClientDbSyntaxQueryInputKind, ClientDbSyntaxQueryLookup,
    ClientDbSyntaxQueryReplay,
};
pub use pragmas::{ClientDbJournalMode, ClientDbRuntimePragmas};
pub use structural_index::{
    ClientDbStructuralDependencyUsage, ClientDbStructuralHash, ClientDbStructuralIndexImport,
    ClientDbStructuralIndexLookup, ClientDbStructuralIndexRefreshPlan,
    ClientDbStructuralIndexStats, ClientDbStructuralKind, ClientDbStructuralLocator,
    ClientDbStructuralName, ClientDbStructuralOwner, ClientDbStructuralPath,
    ClientDbStructuralQueryKey, ClientDbStructuralSource, ClientDbStructuralSymbol,
};
