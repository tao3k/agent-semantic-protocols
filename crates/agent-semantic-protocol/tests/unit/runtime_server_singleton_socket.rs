use std::os::unix::net::UnixDatagram;

#[path = "../../src/command/runtime_server_singleton_socket.rs"]
mod singleton_socket;

#[test]
fn a_second_resident_cannot_own_the_singleton_socket() {
    let state_home = tempfile::tempdir().expect("temporary state home");
    let first = singleton_socket::acquire(state_home.path()).expect("first acquisition");
    assert!(matches!(
        first,
        singleton_socket::AcquireOutcome::Acquired(_)
    ));

    let second = singleton_socket::acquire(state_home.path()).expect("second acquisition");
    assert!(matches!(
        second,
        singleton_socket::AcquireOutcome::ResidentExists
    ));
}

#[test]
fn stale_path_is_observed_before_election_protected_recovery() {
    let state_home = tempfile::tempdir().expect("temporary state home");
    let server_root = state_home.path().join("runtime").join("server");
    std::fs::create_dir_all(&server_root).expect("server root");
    let socket_path = server_root.join("runtime-server-singleton.sock");
    let stale_socket = UnixDatagram::bind(&socket_path).expect("stale socket");
    drop(stale_socket);

    let observed = singleton_socket::acquire(state_home.path()).expect("stale observation");
    assert!(matches!(observed, singleton_socket::AcquireOutcome::Stale));
    assert!(socket_path.exists(), "observation must not unlink stale state");

    let recovered = singleton_socket::acquire_with_stale_recovery_under_election(
        state_home.path(),
    )
    .expect("election-protected recovery");
    assert!(matches!(
        recovered,
        singleton_socket::AcquireOutcome::Acquired(_)
    ));
}
