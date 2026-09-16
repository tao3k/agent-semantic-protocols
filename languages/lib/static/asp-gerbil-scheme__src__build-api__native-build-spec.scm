(declare (block) (standard-bindings) (extended-bindings))
(begin
  (define asp-gerbil-scheme/src/build-api/native-build-spec::timestamp
    1789096524)
  (begin
    (define asp-gerbil-scheme/src/build-api/native-build-spec#package-root '#f)
    (define asp-gerbil-scheme/src/build-api/native-build-spec#source-root '#f)
    (define asp-gerbil-scheme/src/build-api/native-build-spec#current-package-gerbil-modules-key
      '#f)
    (define asp-gerbil-scheme/src/build-api/native-build-spec#current-package-gerbil-modules
      '#f)
    (define asp-gerbil-scheme/src/build-api/native-build-spec#package-name '#f)
    (define asp-gerbil-scheme/src/build-api/native-build-spec#configure-build-root!
      (lambda (_%root7711%_)
        (set! asp-gerbil-scheme/src/build-api/native-build-spec#package-root
              (path-normalize _%root7711%_))
        (asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-configure-build-root!
         asp-gerbil-scheme/src/build-api/native-build-spec#package-root)
        (set! asp-gerbil-scheme/src/build-api/native-build-spec#source-root
              (path-expand
               '"src"
               asp-gerbil-scheme/src/build-api/native-build-spec#package-root))
        (set! asp-gerbil-scheme/src/build-api/native-build-spec#current-package-gerbil-modules-key
              '#f)
        (set! asp-gerbil-scheme/src/build-api/native-build-spec#current-package-gerbil-modules
              '#f)
        (set! asp-gerbil-scheme/src/build-api/native-build-spec#package-name
              (asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-build-package-name
               asp-gerbil-scheme/src/build-api/native-build-spec#package-root))))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#ensure-build-root!
      (lambda ()
        (if asp-gerbil-scheme/src/build-api/native-build-spec#package-root
            '#!void
            (asp-gerbil-scheme/src/build-api/native-build-spec#configure-build-root!
             (current-directory)))))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#package-output-prefix
      (lambda (_%root-name7708%_)
        (asp-gerbil-scheme/src/build-api/native-build-spec#ensure-build-root!)
        (if asp-gerbil-scheme/src/build-api/native-build-spec#package-name
            '#!void
            (error '"gerbil.pkg must declare package: for build output prefix"))
        (string-append
         asp-gerbil-scheme/src/build-api/native-build-spec#package-name
         '"/"
         _%root-name7708%_)))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#source-output-prefix
      (lambda ()
        (asp-gerbil-scheme/src/build-api/native-build-spec#package-output-prefix
         '"src")))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#test-output-prefix
      (lambda ()
        (asp-gerbil-scheme/src/build-api/native-build-spec#package-output-prefix
         '"t")))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#excluded-library-files
      '("provider-server.ss" "commands/provider-runtime.ss"))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#+library-excluded-dirs+
      '("testing"))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#+default-excluded-dirs+
      '("run" "t" ".git" "_darcs" ".gerbil"))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#runtime-library-module?
      (lambda (_%module7704%_)
        (if (not (member _%module7704%_
                         asp-gerbil-scheme/src/build-api/native-build-spec#excluded-library-files))
            (not (asp-gerbil-scheme/src/build-api/native-build-spec#library-excluded-dir-module?
                  _%module7704%_))
            '#f)))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#library-excluded-dir-module?
      (lambda (_%module7695%_)
        (let _%loop7697%_ ((_%dirs7699%_
                            asp-gerbil-scheme/src/build-api/native-build-spec#+library-excluded-dirs+))
          (if (pair? _%dirs7699%_)
              (let ((_%$e7701%_
                     (std/srfi/13#string-prefix?
                      (string-append (car _%dirs7699%_) '"/")
                      _%module7695%_)))
                (if _%$e7701%_ _%$e7701%_ (_%loop7697%_ (cdr _%dirs7699%_))))
              '#f))))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#library-spec
      (lambda ()
        (filter asp-gerbil-scheme/src/build-api/native-build-spec#runtime-library-module?
                (asp-gerbil-scheme/src/build-api/native-build-spec#all-package-gerbil-modules))))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#package-gerbil-modules-cache-key
      (lambda ()
        (list asp-gerbil-scheme/src/build-api/native-build-spec#package-root
              (asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage-roots)
              (asp-gerbil-scheme/src/build-api/native-build-spec#coverage-excluded-directories))))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#all-package-gerbil-modules
      (lambda ()
        (let ((_%key7689%_
               (asp-gerbil-scheme/src/build-api/native-build-spec#package-gerbil-modules-cache-key)))
          (if (and asp-gerbil-scheme/src/build-api/native-build-spec#current-package-gerbil-modules-key
                   (equal? asp-gerbil-scheme/src/build-api/native-build-spec#current-package-gerbil-modules-key
                           _%key7689%_))
              asp-gerbil-scheme/src/build-api/native-build-spec#current-package-gerbil-modules
              (let ((_%modules7691%_
                     (asp-gerbil-scheme/src/build-api/native-build-spec#source-runtime-modules)))
                (set! asp-gerbil-scheme/src/build-api/native-build-spec#current-package-gerbil-modules-key
                      _%key7689%_)
                (set! asp-gerbil-scheme/src/build-api/native-build-spec#current-package-gerbil-modules
                      _%modules7691%_)
                _%modules7691%_)))))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#source-runtime-module-path
      (lambda (_%path7684%_)
        (let ((_%prefix7686%_ '"src/"))
          (if (std/srfi/13#string-prefix? _%prefix7686%_ _%path7684%_)
              (substring
               _%path7684%_
               (string-length _%prefix7686%_)
               (string-length _%path7684%_))
              '#f))))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#source-runtime-modules
      (lambda ()
        (filter (lambda (_%module7682%_) _%module7682%_)
                (map asp-gerbil-scheme/src/build-api/native-build-spec#source-runtime-module-path
                     (asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage-files
                      asp-gerbil-scheme/src/build-api/native-build-spec#package-root)))))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#coverage-excluded-directories
      (lambda ()
        (append asp-gerbil-scheme/src/build-api/native-build-spec#+default-excluded-dirs+
                asp-gerbil-scheme/src/build-api/native-build-spec#+library-excluded-dirs+
                (asp-gerbil-scheme/src/build-api/source-coverage#asp-gerbil-scheme-source-coverage-exclude-directories))))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#package-build-spec
      (lambda ()
        (asp-gerbil-scheme/src/build-api/native-build-spec#ensure-build-root!)
        (asp-gerbil-scheme/src/build-api/package-native-plan#asp-gerbil-scheme-package-api-spec)))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#package-api-output-root
      (lambda ()
        (path-expand
         (asp-gerbil-scheme/src/build-api/native-build-spec#source-output-prefix)
         (path-expand
          '"lib"
          (asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-build-active-gerbil-path
           asp-gerbil-scheme/src/build-api/native-build-spec#package-root)))))
    (define asp-gerbil-scheme/src/build-api/native-build-spec#compile-spec
      (lambda (_%full?7676%_)
        (asp-gerbil-scheme/src/build-api/native-build-spec#ensure-build-root!)
        (if _%full?7676%_
            (asp-gerbil-scheme/src/build-api/native-build-spec#library-spec)
            (asp-gerbil-scheme/src/build-api/package-native-plan#asp-gerbil-scheme-package-api-spec))))))
