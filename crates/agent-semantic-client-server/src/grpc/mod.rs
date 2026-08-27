//! Unix gRPC transport for multiplexed public ASP `ClientFrame` sessions.

mod generated;
mod transport;

pub use transport::{
    AspClientGrpcService, AspClientGrpcTransport, bind_asp_client_grpc_unix,
    serve_asp_client_grpc_unix,
};
