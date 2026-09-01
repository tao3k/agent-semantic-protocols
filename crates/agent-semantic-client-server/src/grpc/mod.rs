//! Unix gRPC transport for multiplexed public ASP `ClientFrame` sessions.

mod generated;
mod transport;
mod wire;

pub use transport::{
    AspClientGrpcService, AspClientGrpcTransport, CLIENT_FRAME_SESSION_CAPACITY,
    CLIENT_FRAME_SESSION_CONTROL_RESERVE, admit_asp_client_grpc_inherited_descriptor,
    bind_asp_client_grpc_unix, connect_asp_client_grpc_inherited_descriptor,
    serve_asp_client_grpc_unix,
};
