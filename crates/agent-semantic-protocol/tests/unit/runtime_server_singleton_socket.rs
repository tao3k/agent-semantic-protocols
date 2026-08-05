use std::os::unix::net::UnixDatagram;

#[path = "../../src/server/runtime_server_singleton_socket.rs"]
mod singleton_socket;

#[tokio::test]
async fn a_second_resident_cannot_own_the_singleton_socket() {
    let state_home = tempfile::tempdir_in("/tmp").expect("short temporary state home");
    let first = singleton_socket::acquire(state_home.path())
        .await
        .expect("first acquisition");
    let _first_guard = match first {
        singleton_socket::SingletonSocketElection::Acquired(guard) => guard,
        singleton_socket::SingletonSocketElection::ResidentExists => {
            panic!("first acquisition must own the singleton socket")
        }
    };

    let second = singleton_socket::acquire(state_home.path())
        .await
        .expect("second acquisition");
    assert!(matches!(
        second,
        singleton_socket::SingletonSocketElection::ResidentExists
    ));
}

#[tokio::test]
async fn stale_path_is_observed_without_mutation_before_election_protected_acquire() {
    let state_home = tempfile::tempdir_in("/tmp").expect("short temporary state home");
    let server_root = state_home.path().join("runtime").join("server");
    std::fs::create_dir_all(&server_root).expect("server root");
    let socket_path = server_root.join("runtime-server-singleton.sock");
    let stale_socket = UnixDatagram::bind(&socket_path).expect("stale socket");
    drop(stale_socket);

    let observed = singleton_socket::resident_exists(state_home.path())
        .await
        .expect("stale observation");
    assert!(!observed);
    assert!(
        socket_path.exists(),
        "observation must not unlink stale state"
    );

    let recovered = singleton_socket::acquire(state_home.path())
        .await
        .expect("election-protected recovery");
    let _recovered_guard = match recovered {
        singleton_socket::SingletonSocketElection::Acquired(guard) => guard,
        singleton_socket::SingletonSocketElection::ResidentExists => {
            panic!("stale pathname recovery must acquire the singleton socket")
        }
    };
}

#[tokio::test]
async fn a_noncanonical_binary_cannot_own_an_installed_state_home() {
    let state_home = tempfile::tempdir().expect("temporary state home");
    let canonical_bin_dir = state_home.path().join("runtime").join("bin");
    std::fs::create_dir_all(&canonical_bin_dir).expect("canonical bin directory");
    std::fs::write(
        canonical_bin_dir.join("asp"),
        b"not the current test binary",
    )
    .expect("fake canonical binary");

    let error = singleton_socket::acquire(state_home.path())
        .await
        .err()
        .expect("noncanonical owner must be rejected");
    assert!(error.contains("refusing non-canonical Runtime Server owner"));
    assert!(error.contains("must use an isolated ASP_STATE_HOME"));
}
