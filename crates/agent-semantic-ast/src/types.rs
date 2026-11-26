// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed domain newtypes, AST span records, calibration receipts, and patch
//! operations for `agent-semantic-ast` (RFC 10.35).

use std::borrow::Cow;
use std::fmt::{self, Display};

use agent_semantic_provider_protocol::ProviderSyntaxSurface;
use serde::{Deserialize, Serialize};

const MAX_IDENTIFIER_LEN: usize = 256;

/// Typed error surface for AST provider registration, inspection, splicing,
/// calibration, and transactional commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AstEngineError {
    InvalidIdentifier(&'static str),
    InvalidSpan(&'static str),
    LocatorNotFound(String),
    PreimageMismatch { expected: String, actual: String },
    ProviderNotRegistered(String),
    DuplicateProvider(String),
    ProviderError(String),
    IoError(String),
}

impl Display for AstEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(msg) => write!(f, "invalid AST identifier: {msg}"),
            Self::InvalidSpan(msg) => write!(f, "invalid UTF-8 byte span: {msg}"),
            Self::LocatorNotFound(loc) => write!(f, "AST node locator not found: {loc}"),
            Self::PreimageMismatch { expected, actual } => {
                write!(
                    f,
                    "stale BLAKE3 preimage: expected {expected}, actual {actual}"
                )
            }
            Self::ProviderNotRegistered(key) => {
                write!(f, "no registered native AST provider for {key}")
            }
            Self::DuplicateProvider(id) => {
                write!(f, "native AST provider already registered: {id}")
            }
            Self::ProviderError(msg) => write!(f, "native AST provider error: {msg}"),
            Self::IoError(msg) => write!(f, "AST transaction I/O error: {msg}"),
        }
    }
}

impl std::error::Error for AstEngineError {}

macro_rules! define_domain_id {
    ($name:ident, $doc:literal, $err:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Construct a validated non-empty identifier.
            pub fn new(raw: impl Into<String>) -> Result<Self, AstEngineError> {
                let value = raw.into();
                if value.trim().is_empty() || value.len() > MAX_IDENTIFIER_LEN {
                    return Err(AstEngineError::InvalidIdentifier($err));
                }
                Ok(Self(value))
            }

            /// Borrow the canonical UTF-8 identifier text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

define_domain_id!(
    AstLanguageId,
    "Canonical language identifier (e.g. `rust`, `typescript`, `python`, `gerbil-scheme`).",
    "language id"
);
define_domain_id!(
    AstLanguageKind,
    "Exact native AST node kind emitted by the language compiler (e.g. `impl_item`, `FunctionDef`, `poo_def`).",
    "language kind"
);
define_domain_id!(
    AstProviderId,
    "Canonical native provider identifier (e.g. `asp-rust`, `asp-typescript`).",
    "provider id"
);
define_domain_id!(
    AstNodeLocator,
    "Structural AST node locator anchored to an owner path (e.g. `src/lib.rs#impl:Engine#fn:run`).",
    "node locator"
);
define_domain_id!(
    Blake3SourceDigest,
    "Canonical `blake3-256:<hex>` content digest of immutable source bytes.",
    "blake3 digest"
);

/// Validated project-relative workspace path disallowing parent escapes or absolute roots.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AstWorkspacePath(String);

impl AstWorkspacePath {
    /// Validate and wrap a relative workspace path.
    pub fn new(raw: impl Into<String>) -> Result<Self, AstEngineError> {
        let value = raw.into();
        let trimmed = value.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('/')
            || trimmed.contains('\\')
            || trimmed.split('/').any(|seg| seg == ".." || seg.is_empty())
        {
            return Err(AstEngineError::InvalidIdentifier("workspace path"));
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the canonical workspace-relative path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract the lowercase leading-dot extension (e.g. `.rs`, `.ts`, `.ss`), if present.
    #[must_use]
    pub fn normalized_extension(&self) -> Option<String> {
        let file_name = self.0.rsplit('/').next()?;
        let (_, ext) = file_name.rsplit_once('.')?;
        if ext.is_empty() {
            return None;
        }
        Some(format!(".{}", ext.to_ascii_lowercase()))
    }
}

/// Half-open UTF-8 byte interval `[start, end)` referencing immutable source bytes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    /// Construct a validated byte span with `start <= end`.
    pub const fn new(start: usize, end: usize) -> Result<Self, AstEngineError> {
        if start > end {
            return Err(AstEngineError::InvalidSpan("start exceeds end"));
        }
        Ok(Self { start, end })
    }

    /// Length in UTF-8 bytes.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end - self.start
    }

    /// Whether the interval is empty (`start == end`).
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Universal cross-language AST node category aligned with `semantic-open-edb.v1`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AstUniversalKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Type,
    Item,
    Macro,
    Module,
    Block,
    Statement,
    Expression,
    Import,
    Constant,
    Heading,
}

impl AstUniversalKind {
    /// Lowercase universal kind matching `semantic-open-edb.v1`.
    #[must_use]
    pub const fn as_open_edb_kind(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Type => "type",
            Self::Item | Self::Block | Self::Statement | Self::Expression => "item",
            Self::Macro => "macro",
            Self::Module => "module",
            Self::Import => "import",
            Self::Constant => "constant",
            Self::Heading => "heading",
        }
    }
}

/// Open key/value facet attached to an inspected AST node.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstNodeAttr {
    pub key: String,
    pub value: String,
}

/// Directed semantic graph edge emitted by a native language compiler
/// (`imports`, `calls`, `implements`, `references`, `contains`).
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstSemanticEdgeRecord {
    pub src: String,
    pub dst: String,
    pub relation: String,
    #[serde(default)]
    pub line: usize,
}

/// Authoritative native AST node span anchored into immutable source bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstNodeSpanRecord {
    pub locator: AstNodeLocator,
    pub kind: AstUniversalKind,
    pub language_kind: AstLanguageKind,
    pub name: String,
    pub line: usize,
    pub outer_span: ByteSpan,
    pub inner_span: ByteSpan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_span: Option<ByteSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<AstNodeLocator>,
    #[serde(default)]
    pub attrs: Vec<AstNodeAttr>,
}

/// Declared AST operations supported by a registered native language provider.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstProviderCapabilities {
    pub ast_inspect: bool,
    pub ast_verify: bool,
    pub open_edb_facts: bool,
}

impl Default for AstProviderCapabilities {
    fn default() -> Self {
        Self {
            ast_inspect: true,
            ast_verify: true,
            open_edb_facts: true,
        }
    }
}

/// Declarative lexical delimiter profile shared across host languages without
/// duplicating language grammars.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LexicalDelimiterProfile {
    pub line_comment: Cow<'static, str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_comment_open: Option<Cow<'static, str>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_comment_close: Option<Cow<'static, str>>,
    pub supports_rust_raw_strings: bool,
    pub supports_single_quote_strings: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement_terminator: Option<char>,
}

impl LexicalDelimiterProfile {
    /// Lexical profile for Rust source files.
    #[must_use]
    pub const fn rust() -> Self {
        Self {
            line_comment: Cow::Borrowed("//"),
            block_comment_open: Some(Cow::Borrowed("/*")),
            block_comment_close: Some(Cow::Borrowed("*/")),
            supports_rust_raw_strings: true,
            supports_single_quote_strings: false,
            statement_terminator: Some(';'),
        }
    }

    /// Lexical profile for TypeScript / JavaScript source files.
    #[must_use]
    pub const fn typescript() -> Self {
        Self {
            line_comment: Cow::Borrowed("//"),
            block_comment_open: Some(Cow::Borrowed("/*")),
            block_comment_close: Some(Cow::Borrowed("*/")),
            supports_rust_raw_strings: false,
            supports_single_quote_strings: true,
            statement_terminator: Some(';'),
        }
    }

    /// Lexical profile for Python source files.
    #[must_use]
    pub const fn python() -> Self {
        Self {
            line_comment: Cow::Borrowed("#"),
            block_comment_open: None,
            block_comment_close: None,
            supports_rust_raw_strings: false,
            supports_single_quote_strings: true,
            statement_terminator: None,
        }
    }

    /// Lexical profile for Gerbil Scheme / S-expression sources.
    #[must_use]
    pub const fn scheme() -> Self {
        Self {
            line_comment: Cow::Borrowed(";"),
            block_comment_open: Some(Cow::Borrowed("#|")),
            block_comment_close: Some(Cow::Borrowed("|#")),
            supports_rust_raw_strings: false,
            supports_single_quote_strings: false,
            statement_terminator: None,
        }
    }

    /// Construct a calibration lexical profile directly from the native
    /// [`ProviderSyntaxSurface`] reported upward by a registered provider.
    #[must_use]
    pub fn from_syntax_surface(surface: &ProviderSyntaxSurface) -> Self {
        Self {
            line_comment: Cow::Owned(surface.line_comment.clone().unwrap_or_default()),
            block_comment_open: surface
                .block_comment
                .as_ref()
                .map(|[open, _]| Cow::Owned(open.clone())),
            block_comment_close: surface
                .block_comment
                .as_ref()
                .map(|[_, close]| Cow::Owned(close.clone())),
            supports_rust_raw_strings: false,
            supports_single_quote_strings: surface
                .string_delimiters
                .iter()
                .any(|delim| delim == "'"),
            statement_terminator: surface
                .statement_terminator
                .as_deref()
                .and_then(|term| term.chars().next()),
        }
    }
}

/// Native parser diagnostic anchored to a UTF-8 byte interval.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstSyntaxDiagnostic {
    pub byte_start: usize,
    pub byte_end: usize,
    pub message: String,
    #[serde(default)]
    pub expected_tokens: Vec<String>,
}

/// Kind of bounded syntactic calibration edit (strictly non-semantic).
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AstCalibrationEditKind {
    StripCodeFence,
    InsertDelimiter,
    DeleteDelimiter,
    InsertTerminator,
}

/// One deterministic syntax calibration edit recorded in a [`CalibrationReceipt`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstCalibrationEdit {
    pub kind: AstCalibrationEditKind,
    pub byte_offset: usize,
    pub text: String,
    pub cost: u32,
}

/// Terminal outcome of Frontier 2 syntax calibration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AstCalibrationOutcome {
    ExactFastPath,
    CalibratedUniqueMinCost,
    RejectedAmbiguousTie,
    RejectedTypedHole,
}

/// Content-addressed proof receipt proving sound, unique minimal calibration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationReceipt {
    pub preimage_hash: Blake3SourceDigest,
    pub raw_hash: Blake3SourceDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibrated_hash: Option<Blake3SourceDigest>,
    pub outcome: AstCalibrationOutcome,
    pub total_cost: u32,
    pub edits: Vec<AstCalibrationEdit>,
    pub oracle_provider_id: AstProviderId,
}

/// Hazel-inspired fail-closed diagnostic emitted whenever calibration or patching rejects.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedHoleDiagnostic {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<AstNodeLocator>,
    pub span: ByteSpan,
    pub reason: String,
    pub expected: Vec<String>,
    pub ambiguous_candidates: usize,
}

/// High-level structural AST edit operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AstPatchOp {
    ReplaceItem {
        target: AstNodeLocator,
        snippet: String,
    },
    AppendToBlock {
        target: AstNodeLocator,
        snippet: String,
    },
    InsertBeforeNode {
        target: AstNodeLocator,
        snippet: String,
    },
    InsertAfterNode {
        target: AstNodeLocator,
        snippet: String,
    },
    InsertImport {
        import_statement: String,
    },
    RemoveNode {
        target: AstNodeLocator,
    },
    ApplySpanEdits {
        edits: Vec<(ByteSpan, String)>,
    },
}

/// Request executing one transactional lossless AST patch via [`crate::AstProviderRegistry`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstPatchRequest {
    pub path: AstWorkspacePath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_id: Option<AstLanguageId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_preimage_hash: Option<Blake3SourceDigest>,
    pub operation: AstPatchOp,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default = "default_true")]
    pub auto_calibrate: bool,
}

const fn default_true() -> bool {
    true
}

/// Terminal status of an [`AstPatchReceipt`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AstPatchStatus {
    Verified,
    Applied,
    CalibratedAndApplied,
    RejectedFailClosed,
}

/// Auditable receipt emitted for every dry-run, calibration, or atomic AST commit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AstPatchReceipt {
    pub status: AstPatchStatus,
    pub path: AstWorkspacePath,
    pub language_id: AstLanguageId,
    pub provider_id: AstProviderId,
    pub preimage_hash: Blake3SourceDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postimage_hash: Option<Blake3SourceDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<CalibrationReceipt>,
    pub reparse_verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_hole: Option<TypedHoleDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unified_diff: Option<String>,
    pub open_edb_ingested_entities: usize,
}
