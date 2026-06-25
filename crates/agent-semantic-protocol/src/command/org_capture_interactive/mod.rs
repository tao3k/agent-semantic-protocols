//! Contract-owned interactive windows for `asp org capture`.

mod choice;

pub(super) use choice::{AgentInteractiveChoice, choice_arg_value, strip_choice_args};
