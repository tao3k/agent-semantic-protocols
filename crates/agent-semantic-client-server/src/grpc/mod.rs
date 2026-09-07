// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Loopback TCP gRPC transport for multiplexed public ASP `ClientFrame` sessions.

mod generated;
mod transport;
mod wire;

pub use transport::AspClientGrpcService;
pub use transport::AspClientGrpcTransport;
pub use transport::CLIENT_FRAME_SESSION_CAPACITY;
pub use transport::CLIENT_FRAME_SESSION_CONTROL_RESERVE;
pub use transport::admit_asp_client_grpc_inherited_descriptor;
pub use transport::bind_asp_client_grpc_tcp;
pub use transport::connect_asp_client_grpc_inherited_descriptor;
pub use transport::serve_asp_client_grpc_tcp;
