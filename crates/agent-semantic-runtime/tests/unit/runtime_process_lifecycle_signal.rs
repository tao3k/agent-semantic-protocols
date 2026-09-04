use super::classify_kill_result;

#[test]
fn missing_process_is_already_stopped() {
    classify_kill_result(-1, std::io::Error::from_raw_os_error(libc::ESRCH)).unwrap();
}

#[test]
fn other_signal_errors_remain_fail_closed() {
    let error =
        classify_kill_result(-1, std::io::Error::from_raw_os_error(libc::EPERM)).unwrap_err();
    assert!(!error.is_empty());
}
