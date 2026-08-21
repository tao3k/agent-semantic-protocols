//! Pure candidate lifecycle policy shared by Runtime-owned adapters.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateState { Accepted, DrainRequested, FailedPrepromotion, Promoting, Other }

impl CandidateState {
    pub fn parse(value: &str) -> Self {
        match value { "accepted" => Self::Accepted, "drain-requested" => Self::DrainRequested, "failed-prepromotion" => Self::FailedPrepromotion, "promoting" => Self::Promoting, _ => Self::Other }
    }
    pub fn may_be_superseded(self) -> bool { matches!(self, Self::Accepted | Self::DrainRequested | Self::FailedPrepromotion) }
    pub fn is_promoting(self) -> bool { self == Self::Promoting }
}
