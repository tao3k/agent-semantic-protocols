// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generated gRPC binding for the public ASP `ClientFrame` stream.

// async_trait emits must_use on a Future that is already must_use.
// Keep this exception inside generated bindings; handwritten code stays checked.
#![allow(clippy::double_must_use)]

tonic::include_proto!("asp.client.protocol.v1");
