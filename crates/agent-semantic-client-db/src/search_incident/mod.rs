mod lifecycle;
mod model;
mod terminal;
pub use lifecycle::{
    apply_replay, begin_repair, compact, observe, reopen_failed_verification, request_verification,
    should_record, supersede,
};
pub use model::{
    IncidentIdentity, IncidentObservation, IncidentRecord, IncidentState, IncidentSurface,
    IncidentTelemetryEvent, ReplayReceipt, RequestedProjection, ResourceObservation,
    TransitionError,
};
pub use terminal::{
    SearchIncidentTerminalContext, SearchIncidentTerminalOutcome, observe_terminal,
};
