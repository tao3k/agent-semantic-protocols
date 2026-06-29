use std::marker::PhantomData;

use super::core::{
    DynamicOverlayDocument, DynamicOverlayNamespace, DynamicOverlayQuery,
    DynamicOverlaySearchBackend, DynamicOverlaySearchHit, InMemoryDynamicOverlaySearch,
};

pub(crate) struct TursoDynamicOverlaySearch {
    control: InMemoryDynamicOverlaySearch,
    _driver_marker: PhantomData<fn() -> turso::Database>,
}

impl Default for TursoDynamicOverlaySearch {
    fn default() -> Self {
        Self {
            control: InMemoryDynamicOverlaySearch::default(),
            _driver_marker: PhantomData,
        }
    }
}

impl DynamicOverlaySearchBackend for TursoDynamicOverlaySearch {
    fn upsert_documents(
        &mut self,
        namespace: DynamicOverlayNamespace,
        documents: Vec<DynamicOverlayDocument>,
    ) {
        self.control.upsert_documents(namespace, documents);
    }

    fn search(
        &self,
        namespace: &DynamicOverlayNamespace,
        query: &DynamicOverlayQuery,
    ) -> Vec<DynamicOverlaySearchHit> {
        self.control.search(namespace, query)
    }
}
