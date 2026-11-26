#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure, language-agnostic AI-Native Language Server AST Backend Runtime for ASP
//! (RFC 10.35):
//!
//! - [`AstProviderRegistry`]: dynamic registration & routing of external native
//!   language compilers (`asp-rust`, `asp-python`, `asp-typescript`, `AspJulia.jl`,
//!   `asp-gerbil-scheme`, or any third-party provider speaking ASP).
//! - Source-Backed Span-Indexed Lossless Overlay (`span_overlay`).
//! - Frontier 2 Oracle-Verified Bounded Syntax Calibration (`calibration`).
//! - Transactional atomic patching & single-tree Open EDB fact streaming
//!   (`transaction`, `open_edb_projection`).
//!
//! All Coding Style policies, `:gerbil-ascent` relational rules, cost lattices,
//! and higher-order code elevation plans belong in the Gerbil Scheme extension
//! plane (`marlin-agent-core/scheme/ast-style/`), never hardcoded inside Rust.

mod calibration;
mod conformance;
mod open_edb_projection;
mod provider_port;
mod registry;
mod span_overlay;
mod tokio_runtime;
mod transaction;
mod types;

pub use calibration::{CalibrationOutcome, CalibrationRequest, calibrate_with_oracle};
pub use conformance::{
    ConformanceProbeReceipt, LanguageConformanceReport, run_language_conformance_pack,
};
pub use open_edb_projection::project_nodes_to_open_edb_batch;
pub use provider_port::{
    AST_INSPECT_OPERATION, AST_VERIFY_OPERATION, AstInspectRequest, AstInspectResponse,
    AstProviderClientConfig, AstProviderPort, AstVerifyCandidate, AstVerifyCandidateResult,
    AstVerifyRequest, AstVerifyResponse, HttpJsonAstProviderClient, ResidentActorAstProvider,
};
pub use registry::{
    AstProviderDescriptor, AstProviderDocumentBinding, AstProviderRegisterReceipt,
    AstProviderRegistry, AstProviderTransportKind, RegisteredAstProvider,
};
pub use span_overlay::{
    SpanReplacement, blake3_source_digest, compute_op_span_edits, render_compact_unified_diff,
    splice_lossless,
};
pub use transaction::{AstEngine, AstFileInspection};
pub use types::{
    AstCalibrationEdit, AstCalibrationEditKind, AstCalibrationOutcome, AstEngineError,
    AstLanguageId, AstLanguageKind, AstNodeAttr, AstNodeLocator, AstNodeSpanRecord, AstPatchOp,
    AstPatchReceipt, AstPatchRequest, AstPatchStatus, AstProviderCapabilities, AstProviderId,
    AstSyntaxDiagnostic, AstUniversalKind, AstWorkspacePath, Blake3SourceDigest, ByteSpan,
    CalibrationReceipt, LexicalDelimiterProfile, TypedHoleDiagnostic,
};
