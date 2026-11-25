// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

// Project model entry. Load this file together with dev/sysml.
// Model map and view commands: dev/sysml/README.org
package AgentSemanticProtocols {
    private import Views::*;
    private import AspSearchEngine::*;

    part <'ASP-PROJECT'> project {
        doc /* Agent Semantic Protocols (ASP): pure-Rust multi-language
               semantic search, tree-sitter query planning, and topology
               inference engine designed for native library embedding in
               Marlin Agent CORE and process/gRPC interop with external
               language providers. */
    }

    dependency searchEngineArchitecture from project to AspSearchEngine::target;
    dependency searchGenerationLifecycle from project to AspSearchEngine::SearchGenerationLifecycle;
    dependency searchHotPath from project to AspSearchEngine::ExecuteSearchPipeline;

    view projectStructure {
        expose project;
        expose AspSearchEngine::actual;
        expose AspSearchEngine::target;
        expose AspSearchEngine::zeroLegacyPureRustEngine;
        expose AspSearchEngine::hostSuppliedPorts;
        expose AspSearchEngine::atomicGenerationCas;
        expose AspSearchEngine::declarativeProviderPackaging;
        expose AspSearchEngine::externalProviderTransportBoundary;
        expose AspSearchEngine::formalAndStatefulVerification;
        expose AspSearchEngine::highPerformanceBenchmarks;
        render asTreeDiagram;
    }

    view searchPipelineFlow : StandardViewDefinitions::ActionFlowView {
        expose AspSearchEngine::ExecuteSearchPipeline;
    }
}
