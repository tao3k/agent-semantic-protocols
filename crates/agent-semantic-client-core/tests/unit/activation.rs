use agent_semantic_hook::RuntimeProviderHealthStatus;

use crate::RuntimeProfileStatus;

#[test]
fn runtime_profile_status_preserves_receipt_labels() {
    assert_eq!(RuntimeProfileStatus::Available.as_str(), "available");
    assert_eq!(RuntimeProfileStatus::Missing.as_str(), "missing");
    assert_eq!(RuntimeProfileStatus::Unexecutable.as_str(), "unexecutable");
}

#[test]
fn runtime_profile_status_maps_from_hook_health_status() {
    assert_eq!(
        RuntimeProfileStatus::from(RuntimeProviderHealthStatus::Available),
        RuntimeProfileStatus::Available
    );
    assert_eq!(
        RuntimeProfileStatus::from(RuntimeProviderHealthStatus::Missing),
        RuntimeProfileStatus::Missing
    );
    assert_eq!(
        RuntimeProfileStatus::from(RuntimeProviderHealthStatus::Unexecutable),
        RuntimeProfileStatus::Unexecutable
    );
}
