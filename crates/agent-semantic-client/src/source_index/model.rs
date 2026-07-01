//! Public data model for DB Engine source-index refresh and lookup receipts.

pub use agent_semantic_client_db::{
    ClientDbSourceIndexCandidate as SourceIndexCandidate,
    ClientDbSourceIndexLookupResult as SourceIndexLookupResult,
    ClientDbSourceIndexLookupState as SourceIndexLookupState,
    ClientDbSourceIndexRefreshResult as SourceIndexRefreshReport,
    ClientDbSourceIndexScopeFile as SourceIndexScopeFile,
    ClientDbSourceIndexSourceKind as SourceIndexSourceKind,
};
