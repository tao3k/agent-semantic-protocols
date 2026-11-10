(declare (block) (standard-bindings) (extended-bindings))
(begin
  (define asp-gerbil-scheme/src/build-api/source-coverage::timestamp
    1789096518)
  (begin
    (define asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-roots
      '("src"))
    (define asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-exclude-directories
      '())
    (define asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-declared-files
      '#f)
    (define asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-owner-root
      '#f)
    (define asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage
      (let ((_%kw-lambda76127653%_
             (let ((_%kw-lambda-main76137646%_
                    (lambda (_%@@keywords7621%_
                             _%roots76147622%_
                             _%exclude-directories76157624%_
                             _%files76167626%_
                             _%owner-root76177628%_
                             _%explanation76187630%_)
                      (let* ((_%roots7633%_
                              (if (eq? _%roots76147622%_ absent-value)
                                  '()
                                  _%roots76147622%_))
                             (_%exclude-directories7635%_
                              (if (eq? _%exclude-directories76157624%_
                                       absent-value)
                                  '()
                                  _%exclude-directories76157624%_))
                             (_%files7637%_
                              (if (eq? _%files76167626%_ absent-value)
                                  '#f
                                  _%files76167626%_))
                             (_%owner-root7639%_
                              (if (eq? _%owner-root76177628%_ absent-value)
                                  '#f
                                  _%owner-root76177628%_))
                             (_%explanation7641%_
                              (if (eq? _%explanation76187630%_ absent-value)
                                  '#f
                                  _%explanation76187630%_)))
                        (set! asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-roots
                              _%roots7633%_)
                        (set! asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-exclude-directories
                              _%exclude-directories7635%_)
                        (set! asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-declared-files
                              (if _%files7637%_
                                  (std/sort#sort _%files7637%_ string<?)
                                  '#f))
                        (set! asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-owner-root
                              (path-normalize
                               (let ((_%$e7643%_ _%owner-root7639%_))
                                 (if _%$e7643%_
                                     _%$e7643%_
                                     (current-directory)))))
                        '#!void))))
               (lambda (_%@@keywords7649%_ . _%args7650%_)
                 (apply _%kw-lambda-main76137646%_
                        _%@@keywords7649%_
                        (symbolic-table-ref
                         _%@@keywords7649%_
                         'roots:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords7649%_
                         'exclude-directories:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords7649%_
                         'files:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords7649%_
                         'owner-root:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords7649%_
                         'explanation:
                         absent-value)
                        _%args7650%_)))))
        (lambda _%args76197656%_
          (apply keyword-dispatch
                 '#(#f
                    #f
                    #f
                    owner-root:
                    files:
                    exclude-directories:
                    #f
                    #f
                    #f
                    #f
                    explanation:
                    #f
                    #f
                    #f
                    roots:)
                 _%kw-lambda76127653%_
                 _%args76197656%_))))
    (define asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-load-source-coverage
      (lambda (_%root7602%_)
        (let ((_%owner-root7604%_ (path-normalize (path-expand _%root7602%_))))
          (if (and asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-declared-files
                   (equal? _%owner-root7604%_
                           asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-owner-root))
              '#!void
              (let* ((_%profile7606%_
                      asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-development-builder-profile)
                     (_%roots7608%_
                      asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-roots)
                     (_%files7610%_
                      (asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-modules/root-roots
                       _%profile7606%_
                       _%owner-root7604%_
                       _%roots7608%_)))
                (asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage
                 'roots:
                 _%roots7608%_
                 'exclude-directories:
                 (asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-exclude-directories
                  _%profile7606%_)
                 'files:
                 _%files7610%_
                 'owner-root:
                 _%owner-root7604%_))))))
    (define asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage-roots
      (lambda ()
        asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-roots))
    (define asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage-exclude-directories
      (lambda ()
        asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-exclude-directories))
    (define asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage-declared-files
      (lambda ()
        asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-declared-files))
    (define asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage-owner-root
      (lambda ()
        asp-gerbil-scheme/src/build-api/source-coverage#current-source-coverage-owner-root))
    (define asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage-files
      (lambda (_%root7593%_)
        (asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-load-source-coverage
         _%root7593%_)
        (let ((_%$e7595%_
               (asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage-declared-files)))
          (if _%$e7595%_
              _%$e7595%_
              (error '"source coverage requires the Build API module catalog")))))))
