//! Rust-owned SQL source index rows for workspace source discovery.

mod api;
mod lookup;
mod storage;
mod text;
mod types;

pub use types::{
    ClientDbSourceIndexImport, ClientDbSourceIndexLookup, ClientDbSourceIndexOwner,
    ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey, ClientDbSourceIndexSelector,
    ClientDbSourceIndexSelectorLookup, ClientDbSourceIndexSource, ClientDbSourceIndexStats,
};
