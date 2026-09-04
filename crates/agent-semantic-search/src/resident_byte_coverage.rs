//! Immutable generation-bound byte coverage for Ready Search requests.
//!
//! Construction may inspect all admitted owner bytes. Queries only intersect
//! resident trigram postings and return a bounded owner set for exact mmap
//! verification by the Runtime data plane.

use std::collections::BTreeSet;
use std::collections::HashMap;

use crate::ResidentSearchAuthority;

pub const RESIDENT_BYTE_GRAM_WIDTH: usize = 3;

pub struct ResidentByteCoverageInput<'a> {
    pub owner_path: String,
    pub authority: Option<ResidentSearchAuthority>,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResidentByteCoverageOwner {
    owner_path: String,
    authority: Option<ResidentSearchAuthority>,
}

#[derive(Debug)]
pub struct ResidentByteCoverageIndex {
    owners: Vec<ResidentByteCoverageOwner>,
    postings: HashMap<[u8; RESIDENT_BYTE_GRAM_WIDTH], Vec<usize>>,
}

impl ResidentByteCoverageIndex {
    pub fn new<'a>(inputs: impl IntoIterator<Item = ResidentByteCoverageInput<'a>>) -> Self {
        let mut owners = Vec::new();
        let mut postings = HashMap::<[u8; RESIDENT_BYTE_GRAM_WIDTH], Vec<usize>>::new();
        for input in inputs {
            let owner_id = owners.len();
            owners.push(ResidentByteCoverageOwner {
                owner_path: input.owner_path,
                authority: input.authority,
            });
            let grams = input
                .bytes
                .windows(RESIDENT_BYTE_GRAM_WIDTH)
                .map(|window| [window[0], window[1], window[2]])
                .collect::<BTreeSet<_>>();
            for gram in grams {
                postings.entry(gram).or_default().push(owner_id);
            }
        }
        Self { owners, postings }
    }

    pub fn candidate_owner_paths(
        &self,
        literal: &[u8],
        authority: Option<&ResidentSearchAuthority>,
        candidate_limit: usize,
    ) -> Result<Vec<String>, String> {
        if literal.len() < RESIDENT_BYTE_GRAM_WIDTH {
            return Err(format!(
                "query-not-ready: exact byte query requires at least {RESIDENT_BYTE_GRAM_WIDTH} bytes"
            ));
        }
        if candidate_limit == 0 {
            return Err("query-not-ready: exact byte candidate budget is zero".to_owned());
        }
        let mut posting_lists = literal
            .windows(RESIDENT_BYTE_GRAM_WIDTH)
            .map(|window| [window[0], window[1], window[2]])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|gram| self.postings.get(&gram).map(Vec::as_slice))
            .collect::<Vec<_>>();
        if posting_lists.iter().any(Option::is_none) {
            return Ok(Vec::new());
        }
        posting_lists.sort_by_key(|posting| posting.map_or(0, <[usize]>::len));
        let mut candidates = posting_lists
            .first()
            .and_then(|posting| *posting)
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|owner_id| {
                posting_lists.iter().skip(1).all(|posting| {
                    posting.is_some_and(|values| values.binary_search(owner_id).is_ok())
                })
            })
            .filter(|owner_id| {
                authority.is_none_or(|required| {
                    self.owners
                        .get(*owner_id)
                        .and_then(|owner| owner.authority.as_ref())
                        == Some(required)
                })
            })
            .collect::<Vec<_>>();
        if candidates.len() > candidate_limit {
            return Err(format!(
                "query-not-ready: exact byte candidate budget exceeded: candidates={} limit={candidate_limit}",
                candidates.len()
            ));
        }
        candidates.sort_unstable();
        Ok(candidates
            .into_iter()
            .filter_map(|owner_id| self.owners.get(owner_id))
            .map(|owner| owner.owner_path.clone())
            .collect())
    }
}
