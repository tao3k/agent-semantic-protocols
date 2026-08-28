//! Contract-owned interactive windows for `asp org capture`.

mod choice;

pub(crate) use choice::{AdmittedAgentInteractiveChoice, AgentInteractiveChoice};
pub(super) use choice::{choice_arg_value, strip_choice_args};
