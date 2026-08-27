mod choice_plane;
mod pane;
mod registration;

#[cfg(test)]
pub(crate) use choice_plane::typed_runtime_failure_reason;
pub(crate) use choice_plane::{CHILD_REGISTRATION_AUTHORITY, CHILD_REGISTRATION_SCHEMA_ID};
#[cfg(test)]
pub(crate) use choice_plane::{
    ChildSessionRegistrationReceipt, child_registration_control_plane_projection,
    child_registration_path, publish_child_registration_receipt_at_path,
    read_child_registration_at_path, read_current_child_registration_at_path,
    read_current_child_registrations_for_route_at_root,
    read_replaceable_child_registration_at_path,
};
pub(crate) use choice_plane::{ChoicePlaneRequest, open_choice_plane};
#[cfg(test)]
pub(crate) use pane::hook_inbox_reconciliation_receipt;
pub(crate) use registration::{
    read_child_session_registration_from_host_payload, register_child_session_from_host_payload,
};
