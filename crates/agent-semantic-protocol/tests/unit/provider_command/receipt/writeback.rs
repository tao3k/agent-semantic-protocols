#[path = "writeback_cases/query_writeback.rs"]
mod query_writeback;
#[path = "writeback_cases/search_miss.rs"]
mod search_miss;
#[path = "writeback_cases/support.rs"]
mod support;
#[path = "writeback_cases/syntax_replay.rs"]
mod syntax_replay;
#[path = "writeback_cases/warm_provider.rs"]
mod warm_provider;

use syntax_replay::attach_cache_file_hashes;
