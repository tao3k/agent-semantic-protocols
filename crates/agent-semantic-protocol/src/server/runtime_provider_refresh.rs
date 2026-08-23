pub(crate) fn provider_runtime_is_stale(
    resident_key: &str,
    resident_language_id: &str,
    resident_provider_id: &str,
    launch_key: &str,
    launch_language_id: &str,
    launch_provider_id: &str,
) -> bool {
    resident_key != launch_key
        && resident_language_id == launch_language_id
        && resident_provider_id == launch_provider_id
}
