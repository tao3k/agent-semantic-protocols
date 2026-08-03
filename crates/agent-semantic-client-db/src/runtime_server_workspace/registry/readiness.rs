/// Typed state of the immutable resident generation currently published for a
/// workspace scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishedWorkspaceGenerationState {
    Ready,
    Missing,
    RecoveryRequired { reason: String },
}
