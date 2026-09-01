use std::io::Write;
use std::os::fd::OwnedFd;

use agent_semantic_client_server::admit_asp_client_grpc_inherited_descriptor;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn inherited_descriptor_admission_uses_the_host_connected_stream() {
    let (client, mut host) = std::os::unix::net::UnixStream::pair()
        .expect("create an already-connected Host transport pair");
    let descriptor: OwnedFd = client.into();

    let mut admitted = admit_asp_client_grpc_inherited_descriptor(descriptor)
        .expect("admit the Host-granted descriptor without bind or connect");
    host.write_all(b"client-frame")
        .expect("write through the Host side of the connected descriptor");

    let mut bytes = [0_u8; 12];
    admitted
        .read_exact(&mut bytes)
        .await
        .expect("read through the admitted descriptor");
    assert_eq!(&bytes, b"client-frame");
}
