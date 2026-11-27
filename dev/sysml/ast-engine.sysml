// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

package AspAstEngine {
    private import ScalarValues::*;

    doc /* OpenSysML v2 engineering architecture for ASP's Rust AST Engine
           (`crates/agent-semantic-ast`, RFC 10.35), unifying:
           1. Source-Backed Span-Indexed Lossless AST Patching across external
              native language providers (`POST /v1/provider-runtime`:
              `ast-inspect` & `ast-verify`) and `:gerbil-parser` Rust AOT.
           2. Frontier 1 — Coding Style + `:gerbil-ascent` Higher-Level Code
              Elevation (pre-gen style contracts, declarative AST smell
              detection, cost-guided semantics-preserving rewrites, and Dusa
              architectural refactoring proposals).
           3. Frontier 2 — Zero-Round-Trip Automatic Syntax Calibration
              (bounded CPCT+ LR recovery and native-oracle-verified delimiter
              calibration with strict uniqueness and fail-closed Typed Holes). */

    enum def AstBackendAuthority {
        enum residentProviderActor;
        enum externalHttpJsonProvider;
        enum embeddedHostPort;
        enum gerbilParserRustAot;
    }

    enum def AstCalibrationOutcome {
        enum exactFastPath;
        enum calibratedUniqueMinCost;
        enum rejectedAmbiguousTie;
        enum rejectedTypedHole;
    }

    enum def StyleElevationClass {
        enum preGenContract;
        enum autoElevateLocal;
        enum proposeAstPatch;
    }

    attribute def AstNodeSpanLocator {
        attribute path : String;
        attribute canonicalLocator : String;
        attribute universalKind : String;
        attribute languageKind : String;
        attribute outerStartByte : Integer;
        attribute outerEndByte : Integer;
        attribute innerStartByte : Integer;
        attribute innerEndByte : Integer;
        doc /* Native-parser-owned UTF-8 byte spans anchored directly into
               immutable BLAKE3-hashed source bytes so untouched trivia and
               comments remain bit-for-bit lossless. */
    }

    attribute def CalibrationReceipt {
        attribute preimageHash : String;
        attribute calibratedHash : String [0..1];
        attribute editCost : Integer;
        attribute uniqueMinimalSolution : Boolean;
        attribute reparseVerified : Boolean;
        attribute outcome : AstCalibrationOutcome;
        doc /* Content-addressed proof receipt proving sound, unique minimal
               syntax calibration verified by the authoritative native parser. */
    }

    part <'ASP-AST-A01'> target {
        part spanOverlayKernel {
            doc /* `crates/agent-semantic-ast/src/span_overlay.rs`: immutable
                   BLAKE3 preimage verification, indentation alignment,
                   non-overlapping reverse-byte-order splicing, and unified
                   audit diff rendering without ever pretty-printing whole ASTs. */
        }
        part providerRegistry {
            doc /* `crates/agent-semantic-ast/src/registry.rs`: AI-Native
                   Language Server AST Provider Register Engine bridging ASP's
                   canonical `ProviderRegistrationDocument` (`languageId`,
                   `providerId`, `sourceInventory.sourceExtensions`) into a
                   language-agnostic dynamic routing table with monotonic
                   generation + BLAKE3 registry digest. Zero hardcoded language
                   parsers (`syn`, `tsc`, `tree-sitter`) inside `agent-semantic-ast`. */
        }
        part nativeProviderPort {
            doc /* `crates/agent-semantic-ast/src/provider_port.rs`: typed
                   `AstProviderPort`, `ResidentActorAstProvider` (binding ASP's
                   `ProviderRuntimeActorClient`), and `HttpJsonAstProviderClient`
                   speaking strictly `POST /v1/provider-runtime` (`ast-inspect`
                   and `ast-verify` ONLY — zero hardcoded style rules in Rust!)
                   across registered `languages/asp-*` providers (`asp-rust`,
                   `asp-typescript`, `asp-python`, `AspJulia.jl`,
                   `asp-gerbil-scheme`). */
        }
        part gerbilParserAdapter {
            doc /* `crates/agent-semantic-ast/src/gerbil_parser_adapter.rs`:
                   in-process adapter over `gerbil-parser-runtime` and
                   `gerbil-parser-artifact` (`ParseArtifact v1` lossless CST +
                   bounded CPCT+ LR recovery) for every `:gerbil-parser`
                   declarative language grammar. */
        }
        part syntaxCalibrator {
            doc /* `crates/agent-semantic-ast/src/calibration.rs`: Frontier 2
                   zero-round-trip syntax calibration combining LR CPCT+
                   (Track A) and token-aware delimiter skeleton + batched
                   native-parser oracle verification (Track B). Enforces strict
                   uniqueness, non-semantic-invention, and fail-closed Typed
                   Hole diagnostics. */
        }
        part styleElevationBridge {
            doc /* Scheme-owned Frontier 1 policy plane (`marlin-agent-core/scheme/ast-style/*`
                   on `:gerbil-ascent` + POO): consumes `AstEngine::inspect_file`
                   Open EDB + AST span facts, evaluates user/agent-authored
                   relational Coding Style rules, projects `Pi_LLM` symbolic
                   contracts, and submits admitted Class-1 `AstPatchOp::ApplySpanEdits`
                   back into the pure Rust backend runtime. */
        }
        part atomicTransactionManager {
            doc /* `crates/agent-semantic-ast/src/transaction.rs`: multi-file
                   in-memory staging, optional formatter execution, mandatory
                   full-file native re-parse proof, atomic commit or zero-write
                   rollback, and `AstPatchReceipt` publication. */
        }
        part openEdbProjector {
            doc /* `crates/agent-semantic-ast/src/open_edb_projection.rs`:
                   projects freshly verified native AST nodes directly into
                   `WorkspaceOpenEdbIngestBatch` (`semantic-open-edb.v1`) so
                   `agent-semantic-search` stays zero-millisecond synchronized. */
        }
        part conformanceHarness {
            doc /* `crates/agent-semantic-ast/src/conformance.rs` &
                   `crates/agent-semantic-provider-protocol/src/provider_core.rs`:
                   Plane 1 Rust `schemars` Provider Core SSoT (version-free
                   filenames) + black-box harness consuming Scheme-projected
                   `LanguageConformancePack` packs and verifying provider-reported
                   `ProviderSyntaxSurface` (`//`, `#`, `;`). */
        }

        dependency txUsesRegistry from atomicTransactionManager to providerRegistry;
        dependency registryUsesProviders from providerRegistry to nativeProviderPort;
        dependency txUsesOverlay from atomicTransactionManager to spanOverlayKernel;
        dependency txUsesCalibrator from atomicTransactionManager to syntaxCalibrator;
        dependency txUsesElevation from atomicTransactionManager to styleElevationBridge;
        dependency txProjectsOpenEdb from atomicTransactionManager to openEdbProjector;
        dependency calibratorUsesProviders from syntaxCalibrator to nativeProviderPort;
        dependency calibratorUsesGerbilParser from syntaxCalibrator to gerbilParserAdapter;
        dependency elevationUsesOverlay from styleElevationBridge to spanOverlayKernel;
        dependency conformanceVerifiesProviders from conformanceHarness to providerRegistry;
    }

    state def AstPatchTransactionLifecycle {
        doc /* Transactional state machine guaranteeing zero partial or
               syntactically broken writes on disk. */
        entry; then idle;

        state idle;
        state inspectingPreimage;
        state calibratingSyntax;
        state elevatingStyle;
        state verifyingReparse;
        state committedAndSynced;
        state rejectedFailClosed;

        transition beginPatch first idle then inspectingPreimage;
        transition preimageStale first inspectingPreimage then rejectedFailClosed;
        transition runCalibration first inspectingPreimage then calibratingSyntax;
        transition calibrationAmbiguousOrInvalid first calibratingSyntax then rejectedFailClosed;
        transition runStyleElevation first calibratingSyntax then elevatingStyle;
        transition verifyFullFile first elevatingStyle then verifyingReparse;
        transition reparseFailed first verifyingReparse then rejectedFailClosed;
        transition commitAtomic first verifyingReparse then committedAndSynced;
        transition resetAfterCommit first committedAndSynced then idle;
        transition resetAfterReject first rejectedFailClosed then idle;
    }

    action def ExecuteAstPatchAndCalibrate {
        in locator : AstNodeSpanLocator;
        out receipt : CalibrationReceipt;

        action inspectNativeSpans {
            doc /* Resolve target AST node spans via authoritative native
                   provider (`ast-inspect`) or `gerbil-parser-runtime` and
                   verify BLAKE3 preimage hash. */
        }
        action calibrateBoundedSyntax {
            doc /* Fast-path valid snippets; on bounded delimiter/punctuation
                   errors, run Track A (LR CPCT+) or Track B (Oracle-Verified
                   Bounded Calibration) enforcing unique minimum-cost proof. */
        }
        action elevateHigherOrderStyle {
            doc /* Project candidate AST to `:gerbil-ascent` Coding Style
                   rules; apply proven Class-1 local elevations and collect
                   Class-2 architectural proposals. */
        }
        action spliceAndVerifyReparse {
            doc /* Reverse-offset byte splice over immutable source copy,
                   optional formatter, and mandatory full-file native
                   `ast-verify` proving zero syntax errors. */
        }
        action commitAndSyncOpenEdb {
            doc /* Atomically write verified files and push updated
                   `semantic-open-edb.v1` facts to `WorkspaceSearchIndex`. */
        }

        first inspectNativeSpans;
        then calibrateBoundedSyntax;
        then elevateHigherOrderStyle;
        then spliceAndVerifyReparse;
        then commitAndSyncOpenEdb;
    }

    requirement <'ASP-AST-R01'> singleNativeParserAuthority {
        doc /* `crates/agent-semantic-ast` MUST delegate language parsing and
               verification to authoritative native language providers over
               `POST /v1/provider-runtime` (`syn`, `tsc`, `ast`, `JuliaSyntax`,
               Gerbil) or `:gerbil-parser` Rust AOT, with zero Tree-sitter and
               zero duplicate third-party language parsers. */
    }
    requirement <'ASP-AST-R02'> losslessSourceBackedSplicing {
        doc /* Structural edits MUST splice UTF-8 byte spans directly against
               immutable BLAKE3-verified source bytes so every untouched
               comment, doc-comment, and formatting byte remains bit-for-bit
               identical, even when backed by AST-only parsers like Rust `syn`
               or Python `ast`. */
    }
    requirement <'ASP-AST-R03'> uniqueMinCostCalibrationSoundness {
        doc /* Frontier 2 auto syntax calibration MUST admit a repair iff the
               minimum-cost edit sequence within budget is unique, invents no
               semantic identifiers/expressions, and passes full authoritative
               native parser verification with zero syntax errors. Ties or deep
               errors MUST fail closed with a Typed Hole diagnostic. */
    }
    requirement <'ASP-AST-R04'> ascentStyleElevationSeparation {
        doc /* Frontier 1 higher-order code elevation MUST separate provably
               semantics-preserving Class-1 local elevations (governed by
               `:gerbil-ascent` + Coding Style cost model) from Class-2
               architectural/API refactorings (emitted as structured proposals,
               never silently mutated). */
    }
    requirement <'ASP-AST-R05'> atomicZeroDirtyRollback {
        doc /* Multi-file AST transactions MUST verify every modified file via
               full re-parse prior to writing and roll back with zero disk
               mutations if any verification, formatting, or preimage check
               fails. */
    }
    requirement <'ASP-AST-R06'> singleTreeOpenEdbSync {
        doc /* Every committed AST transaction MUST project its freshly verified
               native AST nodes directly into `semantic-open-edb.v1` to update
               `WorkspaceSearchIndex` with zero regexes and zero stale window. */
    }
    requirement <'ASP-AST-R07'> comparativeFalsifiableQualification {
        doc /* Both Frontier 1 (style elevation) and Frontier 2 (zero-round-trip
               syntax calibration) MUST be qualified by Quint transaction
               models, Lean CPCT+ minimality proofs, and comparative benchmarks
               against Codex `apply_patch` and Aider `SEARCH/REPLACE`. */
    }
    requirement <'ASP-AST-R08'> twoPlaneSchemaConvergenceAndProviderSyntaxSurface {
        doc /* Plane 1 minimal Provider Core schemas MUST be generated from Rust
               types via `schemars` with version-free filenames (RFC 10.36);
               native providers MUST report their own `ProviderSyntaxSurface`
               upward; Plane 2 Scheme `defasp-language` owns General Language
               Ontology + POO extensions and projects `LanguageConformancePack`
               packs verified by the black-box conformance harness. */
    }

    allocation ownProviderRegistry allocate singleNativeParserAuthority to target.providerRegistry;
    allocation ownNativeAuthority allocate singleNativeParserAuthority to target.nativeProviderPort;
    allocation ownGerbilAuthority allocate singleNativeParserAuthority to target.gerbilParserAdapter;
    allocation ownLosslessSplice allocate losslessSourceBackedSplicing to target.spanOverlayKernel;
    allocation ownCalibrationSoundness allocate uniqueMinCostCalibrationSoundness to target.syntaxCalibrator;
    allocation ownStyleElevation allocate ascentStyleElevationSeparation to target.styleElevationBridge;
    allocation ownAtomicRollback allocate atomicZeroDirtyRollback to target.atomicTransactionManager;
    allocation ownOpenEdbSync allocate singleTreeOpenEdbSync to target.openEdbProjector;
    allocation ownSchemaConvergence allocate twoPlaneSchemaConvergenceAndProviderSyntaxSurface to target.conformanceHarness;
}
