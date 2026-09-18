(declare (block) (standard-bindings) (extended-bindings))
(begin
  (define asp-gerbil-scheme/src/build-api/package-native-plan::timestamp
    1789096521)
  (begin
    (define asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-prologue-stages+
      '(("object-family/syntax.ss")
        ("build-api/package-build.ss" "build-api/build-environment-profile.ss")
        ("build-api/source-discovery.ss" "constants.ss")
        ("build-api/package-receipt.ss"
         "build-api/module-artifacts.ss"
         "build-api/generated-artifact.ss"
         "build-api/builder-profile.ss"
         "support/time.ss")
        ("build-api/source-coverage.ss")
        ("build-api/package-spec.ss" "build-api/package-native-plan.ss")
        ("benchmark/statistics.ss" "benchmark/fixture-model.ss")
        ("benchmark/fixture-contract.ss")
        ("benchmark/gate.ss" "benchmark/micro-kernel.ss")
        ("benchmark/framework.ss")
        ("testing/model.ss")
        ("testing/scope.ss")
        ("testing/scenario.ss" "testing/performance.ss" "testing/batch.ss")
        ("testing/selection.ss")
        ("utilities/functional.ss")
        ("utilities/contracts.ss")
        ("utilities/projection.ss")
        ("utilities/contract-syntax.ss")
        ("types/core.ss"
         "types/env.ss"
         "types/findings.ss"
         "types/source-findings.ss"
         "types/model.ss"
         "types/signatures.ss"
         "types/subtyping.ss"
         "types/validation.ss"
         "types/facade.ss"
         "parser/model.ss"
         "parser/support.ss"
         "parser/formals.ss"
         "parser/syntax-ast.ss"
         "parser/syntax-support.ss"
         "parser/definition-syntax.ss"
         "parser/exact-owner.ss"
         "parser/syntax-calls.ss"
         "parser/imports.ss"
         "parser/syntax.ss"
         "parser/comment-quality-classifier.ss"
         "parser/comment-quality.ss"
         "parser/control-flow.ss"
         "parser/dependency-adapter-quality.ss"
         "parser/exports.ss"
         "parser/higher-order.ss"
         "parser/function-quality.ss"
         "parser/package.ss"
         "parser/profile.ss"
         "parser/quality-shape.ss"
         "parser/selectors.ss"
         "parser/source-scope.ss"
         "parser/source-class.ss"
         "parser/source-file.ss"
         "parser/test-source-scope.ss"
         "parser/typed-contract-token.ss"
         "parser/typed-contract-scheme.ss"
         "parser/runtime-contract.ss"
         "parser/typed-comment-metadata.ss"
         "parser/typed-contract-diagnostics.ss"
         "parser/typed-contract.ss"
         "parser/poo.ss"
         "parser/parse-workers.ss"
         "parser/core.ss"
         "parser/facade.ss"
         "macro-governance/model.ss"
         "extensions/poo-pattern-support.ss"
         "extensions/poo-pattern-typeclass.ss"
         "extensions/poo-patterns.ss")
        ("exact-source-projection.ss")
        ("testing/commands.ss")
        ("testing/framework.ss")))
    (define asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-policy-stages+
      '(("policy/model.ss"
         "policy/agent-support.ss"
         "policy/agent-import.ss"
         "policy/agent-style-steering.ss"
         "policy/agent-style-gerbil-signal-support.ss"
         "policy/agent-style-destructuring-signals.ss"
         "policy/agent-style-performance-signals.ss"
         "policy/agent-style-message.ss"
         "policy/prototype.ss"
         "policy/agent-poo-callees.ss")
        ("policy/agent-alist-access.ss"
         "policy/agent-anonymous-pair.ss"
         "policy/agent-comment.ss"
         "policy/dependency-adapter-profile.ss"
         "policy/agent-list-growth.ss"
         "policy/agent-list-random-access.ss"
         "policy/agent-macro-io.ss"
         "policy/agent-source-scope.ss"
         "policy/agent-string-growth.ss"
         "policy/agent-style-gerbil-boundary-signals.ss"
         "policy/agent-style-gerbil-macro-signals.ss"
         "policy/agent-style-docs.ss"
         "policy/detection.ss"
         "policy/agent-poo-object-literal.ss"
         "policy/agent-build.ss"
         "policy/modularity.ss"
         "policy/catalog.ss")
        ("policy/poo-source.ss"
         "policy/agent-dependency-adapter.ss"
         "policy/agent-style-gerbil-signals.ss"
         "policy/gerbil-utils-source.ss"
         "policy/agent-poo-loop-performance.ss"
         "policy/repair.ss")
        ("policy/agent-package-build-system.ss"
         "policy/agent-style-shape.ss"
         "policy/agent-style-quality.ss"
         "policy/agent-style-details.ss"
         "policy/agent-macro-protocol.ss"
         "policy/agent-poo.ss")
        ("macro-governance/admission.ss")
        ("policy/agent-basic.ss"
         "policy/agent-build-runtime.ss"
         "policy/agent-style.ss")
        ("policy/agent.ss")
        ("policy/core.ss")
        ("macro-governance/facade.ss")
        ("policy/facade.ss")
        ("policy/gxtest-report.ss")))
    (define asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-epilogue-stages+
      '(("testing/build-paths.ss"
         "testing/gxtest-smoke.ss"
         "testing/gxtest-context.ss"
         "testing/gxtest-report.ss")
        ("testing/build-process.ss")
        ("testing/gxtest-syntax.ss")
        ("testing/memory-profile.ss" "testing/execution-profile.ss")
        ("testing/gxtest-imports.ss")
        ("testing/gxtest-sources.ss")
        ("testing/gxtest-discovery.ss")
        ("testing/build-support.ss" "testing/build.ss")
        ("testing/gxtest-delegate.ss")
        ("testing/gxtest-expression.ss")
        ("testing/gxtest-receipts.ss")
        ("testing/gxtest-policy.ss" "testing/gxtest-build.ss")
        ("testing/gxtest-execution.ss")
        ("testing/gxtest-run.ss")
        ("testing/build-runtime.ss")
        ("testing/build-runner.ss")
        ("testing/gxtest-runner.ss")
        ("testing/project-build.ss")
        ("build-api/project-build.ss")
        ("build-api/project-cli.ss")))
    (define asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-command-prologue-stages+
      '(("support/args.ss" "support/io.ss")))
    (define asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-building-stages+
      '(("building/model.ss"
         "building/native-toolchain.ss"
         "building/memory-anomaly-guard.ss")
        ("building/build-script.ss")
        ("building/std-builder.ss")
        ("building/observability.ss")
        ("building/facade.ss")
        ("building/declarative.ss" "building/commands.ss")
        ("testing/building.ss")))
    (define asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-build-api-stages+
      '(("build-api/artifact-cleanup.ss" "build-api/source-closure.ss")
        ("build-api/native-build-spec.ss")
        ("build-api/native-build.ss")
        ("build-api/profile-build-spec.ss")
        ("build-api/framework.ss")))
    (define asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-directory-stages+
      '(("utilities/contract-syntax.ss"
         "utilities/contracts.ss"
         "utilities/functional.ss"
         "utilities/projection.ss")
        ("types/core.ss"
         "types/env.ss"
         "types/facade.ss"
         "types/findings.ss"
         "types/model-syntax.ss"
         "types/model.ss"
         "types/signatures.ss"
         "types/source-findings.ss"
         "types/subtyping.ss"
         "types/validation-union.ss"
         "types/validation.ss")
        ("parser/comment-quality-classifier.ss"
         "parser/comment-quality.ss"
         "parser/control-flow.ss"
         "parser/core.ss"
         "parser/definition-syntax.ss"
         "parser/dependency-adapter-quality.ss"
         "parser/exact-owner.ss"
         "parser/exports.ss"
         "parser/facade.ss"
         "parser/formals.ss"
         "parser/function-quality-signals.ss"
         "parser/function-quality.ss"
         "parser/higher-order.ss"
         "parser/imports.ss"
         "parser/language-projection.ss"
         "parser/model.ss"
         "parser/owner-items.ss"
         "parser/package.ss"
         "parser/parse-workers.ss"
         "parser/poo.ss"
         "parser/profile.ss"
         "parser/quality-shape-loop-driver.ss"
         "parser/quality-shape.ss"
         "parser/query.ss"
         "parser/reader.ss"
         "parser/runtime-contract.ss"
         "parser/selectors.ss"
         "parser/source-class.ss"
         "parser/source-file.ss"
         "parser/source-scope.ss"
         "parser/support.ss"
         "parser/syntax-ast.ss"
         "parser/syntax-calls.ss"
         "parser/syntax-support.ss"
         "parser/syntax.ss"
         "parser/test-source-scope.ss"
         "parser/typed-comment-metadata.ss"
         "parser/typed-contract-comment-index.ss"
         "parser/typed-contract-diagnostics.ss"
         "parser/typed-contract-scheme-expression.ss"
         "parser/typed-contract-scheme.ss"
         "parser/typed-contract-token.ss"
         "parser/typed-contract.ss")
        ("policy/agent-alist-access.ss"
         "policy/agent-anonymous-pair.ss"
         "policy/agent-basic.ss"
         "policy/agent-build-runtime.ss"
         "policy/agent-build.ss"
         "policy/agent-comment.ss"
         "policy/agent-dependency-adapter.ss"
         "policy/agent-import.ss"
         "policy/agent-list-growth.ss"
         "policy/agent-list-random-access.ss"
         "policy/agent-macro-io.ss"
         "policy/agent-macro-protocol.ss"
         "policy/agent-package-build-system.ss"
         "policy/agent-poo-callees.ss"
         "policy/agent-poo-loop-performance.ss"
         "policy/agent-poo-object-literal.ss"
         "policy/agent-poo.ss"
         "policy/agent-source-scope.ss"
         "policy/agent-string-growth.ss"
         "policy/agent-style-destructuring-signals.ss"
         "policy/agent-style-details.ss"
         "policy/agent-style-docs.ss"
         "policy/agent-style-gerbil-boundary-signals.ss"
         "policy/agent-style-gerbil-macro-signals.ss"
         "policy/agent-style-gerbil-signal-support.ss"
         "policy/agent-style-gerbil-signals.ss"
         "policy/agent-style-message.ss"
         "policy/agent-style-performance-signals.ss"
         "policy/agent-style-quality.ss"
         "policy/agent-style-shape.ss"
         "policy/agent-style-steering.ss"
         "policy/agent-style.ss"
         "policy/agent-support.ss"
         "policy/agent.ss"
         "policy/catalog.ss"
         "policy/core.ss"
         "policy/dependency-adapter-profile.ss"
         "policy/detection.ss"
         "policy/facade.ss"
         "policy/gerbil-utils-source.ss"
         "policy/gxtest-report.ss"
         "policy/gxtest-runtime.ss"
         "policy/gxtest.ss"
         "policy/model.ss"
         "policy/modularity.ss"
         "policy/poo-source.ss"
         "policy/prototype.ss"
         "policy/repair-calibration.ss"
         "policy/repair-diagnostic.ss"
         "policy/repair-report.ss"
         "policy/repair.ss"
         "policy/streaming.ss")
        ("macro-governance/admission.ss"
         "macro-governance/facade.ss"
         "macro-governance/model.ss")
        ("protocol/function-quality-facts.ss"
         "protocol/json-output.ss"
         "protocol/json.ss"
         "protocol/quality-shape-facts.ss"
         "protocol/registry.ss"
         "protocol/structural-facts.ss"
         "protocol/structural-index.ss"
         "protocol/support.ss")
        ("extensions/core.ss"
         "extensions/facade.ss"
         "extensions/model.ss"
         "extensions/poo-inheritance.ss"
         "extensions/poo-object-validation.ss"
         "extensions/poo-pattern-support.ss"
         "extensions/poo-pattern-typeclass.ss"
         "extensions/poo-patterns.ss"
         "extensions/poo-source-ref-validation.ss"
         "extensions/poo-validation.ss"
         "extensions/poo.ss")
        ("language/capability.ss"
         "language/compare.ss"
         "language/evidence.ss"
         "language/facade.ss")
        ("format/core.ss" "format/facade.ss" "format/files.ss")
        ("commands/agent.ss"
         "commands/fmt.ss"
         "commands/guide-sections.ss"
         "commands/guide.ss"
         "commands/info.ss"
         "commands/project-resolution.ss"
         "commands/projection-batch.ss"
         "commands/projection.ss"
         "commands/provider-runtime.ss")))
    (define asp-gerbil-scheme/src/build-api/package-native-plan#flatten-stages
      (lambda (_%stages7672%_)
        (std/srfi/1#append-map
         (lambda (_%stage7674%_) _%stage7674%_)
         _%stages7672%_)))
    (define asp-gerbil-scheme/src/build-api/package-native-plan#package-api-stage-specs-cache
      '#f)
    (define asp-gerbil-scheme/src/build-api/package-native-plan#package-api-spec-cache
      '#f)
    (define asp-gerbil-scheme/src/build-api/package-native-plan#package-api-stage-specs/fresh
      (lambda ()
        (append asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-prologue-stages+
                asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-policy-stages+
                asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-building-stages+
                asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-build-api-stages+
                asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-command-prologue-stages+
                asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-directory-stages+
                asp-gerbil-scheme/src/build-api/package-native-plan#+package-api-epilogue-stages+)))
    (define asp-gerbil-scheme/src/build-api/package-native-plan#asp-gerbil-scheme-package-api-stage-specs
      (lambda ()
        (let ((_%$e7666%_
               asp-gerbil-scheme/src/build-api/package-native-plan#package-api-stage-specs-cache))
          (if _%$e7666%_
              _%$e7666%_
              (let ((_%stages7669%_
                     (asp-gerbil-scheme/src/build-api/package-native-plan#package-api-stage-specs/fresh)))
                (set! asp-gerbil-scheme/src/build-api/package-native-plan#package-api-stage-specs-cache
                      _%stages7669%_)
                _%stages7669%_)))))
    (define asp-gerbil-scheme/src/build-api/package-native-plan#asp-gerbil-scheme-package-api-spec
      (lambda ()
        (let ((_%$e7660%_
               asp-gerbil-scheme/src/build-api/package-native-plan#package-api-spec-cache))
          (if _%$e7660%_
              _%$e7660%_
              (let ((_%spec7663%_
                     (asp-gerbil-scheme/src/build-api/package-native-plan#flatten-stages
                      (asp-gerbil-scheme/src/build-api/package-native-plan#asp-gerbil-scheme-package-api-stage-specs))))
                (set! asp-gerbil-scheme/src/build-api/package-native-plan#package-api-spec-cache
                      _%spec7663%_)
                _%spec7663%_)))))))
