//! Shared command-prefix matching for hook rules and execution lanes.
//!
//! Wrapper matching is deliberately lexical: it never probes the filesystem,
//! resolves `PATH`, or starts another process. Every shell stage is scanned
//! for a bounded prefix window so bare commands, absolute executables, and
//! wrapper-prefixed commands have identical routing semantics.

pub const MAX_STAGE_TOKENS: usize = 256;
pub const MAX_PREFIX_WINDOWS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixMatch {
    Matched,
    NotMatched,
    BudgetExceeded,
}

impl PrefixMatch {
    /// Budget exhaustion is protected routing, never an escape hatch.
    pub const fn routes_protected(self) -> bool {
        !matches!(self, Self::NotMatched)
    }
}

pub fn command_stage_matches_prefix(tokens: &[String], prefix: &[String]) -> PrefixMatch {
    if prefix.is_empty() {
        return PrefixMatch::Matched;
    }

    let mut inspected_windows = 0usize;
    for stage in tokens.split(|token| is_shell_stage_separator(token)) {
        if stage.len() > MAX_STAGE_TOKENS {
            return PrefixMatch::BudgetExceeded;
        }
        if stage.len() < prefix.len() {
            continue;
        }

        for candidate in stage.windows(prefix.len()) {
            if inspected_windows == MAX_PREFIX_WINDOWS {
                return PrefixMatch::BudgetExceeded;
            }
            inspected_windows += 1;
            if candidate_matches_prefix(candidate, prefix) {
                return PrefixMatch::Matched;
            }
        }
    }

    PrefixMatch::NotMatched
}

pub fn candidate_matches_prefix(candidate: &[String], prefix: &[String]) -> bool {
    candidate.len() >= prefix.len()
        && candidate
            .iter()
            .zip(prefix)
            .enumerate()
            .all(|(index, (actual, expected))| {
                actual.eq_ignore_ascii_case(expected)
                    || (index == 0 && command_token_basename(actual).eq_ignore_ascii_case(expected))
            })
}

fn command_token_basename(token: &str) -> &str {
    token.rsplit(['/', '\\']).next().unwrap_or(token)
}

fn is_shell_stage_separator(token: &str) -> bool {
    matches!(token, "&&" | "||" | ";" | "|" | "&")
}
