(declare (block) (standard-bindings) (extended-bindings))
(begin
  (define asp-gerbil-scheme/t/package-build-contract-test::timestamp
    1789096528)
  (define asp-gerbil-scheme/t/package-build-contract-test#package-build-contract-test
    (std/test#make-test-suite
     '"asp gerbil-scheme package build contract"
     (lambda ()
       (std/test#run-test-case!
        '"build root preserves the caller Gerbil path"
        (lambda ()
          (let ((_%caller-gerbil-path48230%_ (getenv '"GERBIL_PATH" '#f))
                (_%sentinel48231%_
                 '"/tmp/asp-gerbil-scheme-caller-path-sentinel"))
            (setenv '"GERBIL_PATH" _%sentinel48231%_)
            (asp-gerbil-scheme/src/build-api/native-build-spec#configure-build-root!
             (current-directory))
            (let ((_%val48233%_ _%sentinel48231%_))
              (std/test#verbose
               '"... check ~a is ~a to ~s~n"
               '(getenv "GERBIL_PATH" #f)
               'equal?
               _%val48233%_)
              (std/test#test-check-e
               '(check equal? (getenv "GERBIL_PATH" #f) sentinel)
               std/test#equal-values?
               (lambda () (getenv '"GERBIL_PATH" '#f))
               _%val48233%_
               '"\"package-build-contract-test.ss\"@25.16-25.41"))
            (setenv '"GERBIL_PATH"
                    (let ((_%$e48237%_ _%caller-gerbil-path48230%_))
                      (if _%$e48237%_ _%$e48237%_ '"")))
            (asp-gerbil-scheme/src/build-api/native-build-spec#configure-build-root!
             (current-directory)))))
       (std/test#run-test-case!
        '"empty caller Gerbil path resolves to the Gerbil default"
        (lambda ()
          (let ((_%caller-gerbil-path48241%_ (getenv '"GERBIL_PATH" '#f)))
            (setenv '"GERBIL_PATH" '"")
            (let ((_%val48243%_ (path-expand (gerbil-home))))
              (std/test#verbose
               '"... check ~a is ~a to ~s~n"
               '(asp-gerbil-scheme-package-build-active-gerbil-path
                 (current-directory))
               'equal?
               _%val48243%_)
              (std/test#test-check-e
               '(check equal?
                       (asp-gerbil-scheme-package-build-active-gerbil-path
                        (current-directory))
                       (path-expand (gerbil-home)))
               std/test#equal-values?
               (lambda ()
                 (asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-build-active-gerbil-path
                  (current-directory)))
               _%val48243%_
               '"\"package-build-contract-test.ss\"@31.16-32.37"))
            (setenv '"GERBIL_PATH"
                    (let ((_%$e48247%_ _%caller-gerbil-path48241%_))
                      (if _%$e48247%_ _%$e48247%_ '"")))
            (asp-gerbil-scheme/src/build-api/native-build-spec#configure-build-root!
             (current-directory)))))
       (std/test#run-test-case!
        '"package test driver dependencies remain materialized"
        (lambda ()
          (let ((_%modules48251%_
                 (apply append
                        (asp-gerbil-scheme/src/build-api/package-native-plan#asp-gerbil-scheme-package-api-stage-specs))))
            (std/test#verbose
             '"... check ~a is ~a~n"
             '(member "testing/commands.ss" modules)
             '(? true))
            (std/test#test-check-predicate
             '(check-predicate (member "testing/commands.ss" modules) (? true))
             (lambda () (member '"testing/commands.ss" _%modules48251%_))
             (lambda (_%$obj48254%_) (true _%$obj48254%_))
             '"\"package-build-contract-test.ss\"@38.16-38.54")
            (std/test#verbose
             '"... check ~a is ~a~n"
             '(member "testing/project-build.ss" modules)
             '(? true))
            (std/test#test-check-predicate
             '(check-predicate
               (member "testing/project-build.ss" modules)
               (? true))
             (lambda () (member '"testing/project-build.ss" _%modules48251%_))
             (lambda (_%$obj48258%_) (true _%$obj48258%_))
             '"\"package-build-contract-test.ss\"@39.16-39.59")
            (std/test#verbose
             '"... check ~a is ~a~n"
             '(member "build-api/project-build.ss" modules)
             '(? true))
            (std/test#test-check-predicate
             '(check-predicate
               (member "build-api/project-build.ss" modules)
               (? true))
             (lambda ()
               (member '"build-api/project-build.ss" _%modules48251%_))
             (lambda (_%$obj48262%_) (true _%$obj48262%_))
             '"\"package-build-contract-test.ss\"@40.16-40.61")
            (std/test#verbose
             '"... check ~a is ~a~n"
             '(member "build-api/generated-artifact.ss" modules)
             '(? true))
            (std/test#test-check-predicate
             '(check-predicate
               (member "build-api/generated-artifact.ss" modules)
               (? true))
             (lambda ()
               (member '"build-api/generated-artifact.ss" _%modules48251%_))
             (lambda (_%$obj48266%_) (true _%$obj48266%_))
             '"\"package-build-contract-test.ss\"@41.16-41.66")
            (std/test#verbose
             '"... check ~a is ~a~n"
             '(member "runtime/provider/types.ss" modules)
             '(? true))
            (std/test#test-check-predicate
             '(check-predicate
               (member "runtime/provider/types.ss" modules)
               (? true))
             (lambda () (member '"runtime/provider/types.ss" _%modules48251%_))
             (lambda (_%$obj48270%_) (true _%$obj48270%_))
             '"\"package-build-contract-test.ss\"@42.16-42.60")
            (std/test#verbose
             '"... check ~a is ~a~n"
             '(member "runtime/provider/objects.ss" modules)
             '(? true))
            (std/test#test-check-predicate
             '(check-predicate
               (member "runtime/provider/objects.ss" modules)
               (? true))
             (lambda ()
               (member '"runtime/provider/objects.ss" _%modules48251%_))
             (lambda (_%$obj48274%_) (true _%$obj48274%_))
             '"\"package-build-contract-test.ss\"@43.16-43.62"))))
       (std/test#run-test-case!
        '"package bootstrap compiles native-build dependencies first"
        (lambda ()
          (let _%loop48278%_ ((_%stages48280%_
                               (asp-gerbil-scheme/src/build-api/package-native-plan#asp-gerbil-scheme-package-api-stage-specs))
                              (_%index48281%_ '0)
                              (_%package-build-index48282%_ '#f)
                              (_%native-build-index48283%_ '#f))
            (if (null? _%stages48280%_)
                (begin
                  (let ((_%val48285%_ '#t))
                    (std/test#verbose
                     '"... check ~a is ~a to ~s~n"
                     '(integer? package-build-index)
                     'equal?
                     _%val48285%_)
                    (std/test#test-check-e
                     '(check equal? (integer? package-build-index) #t)
                     std/test#equal-values?
                     (lambda () (integer? _%package-build-index48282%_))
                     _%val48285%_
                     '"\"package-build-contract-test.ss\"@51.20-51.50"))
                  (let ((_%val48289%_ '#t))
                    (std/test#verbose
                     '"... check ~a is ~a to ~s~n"
                     '(< package-build-index native-build-index)
                     'equal?
                     _%val48289%_)
                    (std/test#test-check-e
                     '(check equal?
                             (< package-build-index native-build-index)
                             #t)
                     std/test#equal-values?
                     (lambda ()
                       (< _%package-build-index48282%_
                          _%native-build-index48283%_))
                     _%val48289%_
                     '"\"package-build-contract-test.ss\"@52.20-52.62")))
                (let ((_%stage48293%_ (car _%stages48280%_)))
                  (_%loop48278%_
                   (cdr _%stages48280%_)
                   (+ _%index48281%_ '1)
                   (let ((_%$e48295%_ _%package-build-index48282%_))
                     (if _%$e48295%_
                         _%$e48295%_
                         (if (member '"build-api/package-build.ss"
                                     _%stage48293%_)
                             _%index48281%_
                             '#f)))
                   (let ((_%$e48298%_ _%native-build-index48283%_))
                     (if _%$e48298%_
                         _%$e48298%_
                         (if (member '"build-api/native-build.ss"
                                     _%stage48293%_)
                             _%index48281%_
                             '#f)))))))))
       (std/test#run-test-case!
        '"package build API materializes native std/make owners only"
        (lambda ()
          (let ((_%modules48302%_
                 (apply append
                        (asp-gerbil-scheme/src/build-api/package-native-plan#asp-gerbil-scheme-package-api-stage-specs))))
            (for-each
             (lambda (_%module48304%_)
               (std/test#verbose
                '"... check ~a is ~a~n"
                '(member module modules)
                '(? true))
               (std/test#test-check-predicate
                '(check-predicate (member module modules) (? true))
                (lambda () (member _%module48304%_ _%modules48302%_))
                (lambda (_%$obj48307%_) (true _%$obj48307%_))
                '"\"package-build-contract-test.ss\"@64.19-64.42"))
             '("building/native-toolchain.ss"
               "building/model.ss"
               "building/std-builder.ss"
               "building/facade.ss"
               "building/declarative.ss"))
            (let ((_%val48310%_ '#f))
              (std/test#verbose
               '"... check ~a is ~a to ~s~n"
               '(member "build-api/source-coverage-query.ss" modules)
               'equal?
               _%val48310%_)
              (std/test#test-check-e
               '(check equal?
                       (member "build-api/source-coverage-query.ss" modules)
                       #f)
               std/test#equal-values?
               (lambda ()
                 (member '"build-api/source-coverage-query.ss"
                         _%modules48302%_))
               _%val48310%_
               '"\"package-build-contract-test.ss\"@70.16-70.69")))))))))
