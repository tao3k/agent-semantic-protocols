(declare (block) (standard-bindings) (extended-bindings))
(begin
  (define asp-gerbil-scheme/src/build-api/source-discovery::timestamp
    1789096505)
  (begin
    (define asp-gerbil-scheme/src/build-api/source-discovery#+default-excluded-module-files+
      '("main.ss" "manifest.ss"))
    (define asp-gerbil-scheme/src/build-api/source-discovery#+default-project-exclude-directories+
      '("run" ".git" "_darcs" ".gerbil"))
    (define asp-gerbil-scheme/src/build-api/source-discovery#path-under-excluded-directory?
      (lambda (_%path2433%_ _%directory2434%_)
        (std/srfi/13#string-prefix?
         (string-append _%directory2434%_ '"/")
         _%path2433%_)))
    (define asp-gerbil-scheme/src/build-api/source-discovery#project-module-file?
      (lambda (_%root2424%_ _%path2425%_ _%exclude2426%_ _%exclude-dirs2427%_)
        (if (not (member _%path2425%_ _%exclude2426%_))
            (if (not (ormap (lambda (_%g24282430%_)
                              (asp-gerbil-scheme/src/build-api/source-discovery#path-under-excluded-directory?
                               _%path2425%_
                               _%g24282430%_))
                            _%exclude-dirs2427%_))
                (not (clan/filesystem#path-is-script?
                      (path-expand _%path2425%_ _%root2424%_)))
                '#f)
            '#f)))
    (define asp-gerbil-scheme/src/build-api/source-discovery#git-project?
      (lambda (_%root2411%_)
        (if (file-exists? (path-expand '".git" _%root2411%_))
            (with-catch
             (lambda (_%_2413%_) '#f)
             (lambda ()
               (let* ((_%status2416%_ '0)
                      (_%answer2421%_
                       (std/misc/process#run-process
                        (cons '"git"
                              (cons '"rev-parse"
                                    (cons '"--show-toplevel" '())))
                        'directory:
                        _%root2411%_
                        'coprocess:
                        read-line
                        'stderr-redirection:
                        '#t
                        'check-status:
                        (lambda (_%value2418%_ _%_settings2419%_)
                          (set! _%status2416%_ _%value2418%_)))))
                 (if (zero? _%status2416%_)
                     (if (string? _%answer2421%_)
                         (string=?
                          (path-normalize (path-expand '"." _%root2411%_))
                          (path-normalize
                           (path-expand
                            '"."
                            (std/misc/string#string-trim-eol _%answer2421%_))))
                         '#f)
                     '#f))))
            '#f)))
    (define asp-gerbil-scheme/src/build-api/source-discovery#upstream-project-modules
      (lambda (_%root2406%_ _%exclude2407%_ _%exclude-dirs2408%_)
        (call-with-parameters
         (lambda ()
           (std/sort#sort
            (clan/building#all-gerbil-modules
             'exclude:
             _%exclude2407%_
             'exclude-dirs:
             _%exclude-dirs2408%_)
            string<?))
         current-directory
         _%root2406%_)))
    (define asp-gerbil-scheme/src/build-api/source-discovery#git-ignored-modules
      (lambda (_%root2396%_ _%modules2397%_)
        (with-catch
         (lambda (_%_2399%_) '())
         (lambda ()
           (std/misc/process#filter-with-process
            (cons '"git"
                  (cons '"-c"
                        (cons '"core.quotePath=false"
                              (cons '"check-ignore" (cons '"--stdin" '())))))
            (lambda (_%port2402%_)
              (for-each
               (lambda (_%module2404%_)
                 (display _%module2404%_ _%port2402%_)
                 (newline _%port2402%_))
               _%modules2397%_))
            std/misc/ports#read-all-as-lines
            'directory:
            _%root2396%_)))))
    (define asp-gerbil-scheme/src/build-api/source-discovery#without-ignored-modules
      (lambda (_%modules2385%_ _%ignored2386%_)
        (let ((_%ignored?2388%_ (make-hash-table)))
          (for-each
           (lambda (_%g23892391%_)
             (hash-put! _%ignored?2388%_ _%g23892391%_ '#t))
           _%ignored2386%_)
          (filter (lambda (_%module2394%_)
                    (not (hash-key? _%ignored?2388%_ _%module2394%_)))
                  _%modules2385%_))))
    (define asp-gerbil-scheme/src/build-api/source-discovery#effective-project-exclude-directories
      (lambda (_%default-project-excludes?2382%_ _%exclude-dirs2383%_)
        (append (if _%default-project-excludes?2382%_
                    asp-gerbil-scheme/src/build-api/source-discovery#+default-project-exclude-directories+
                    '())
                _%exclude-dirs2383%_)))
    (define asp-gerbil-scheme/src/build-api/source-discovery#source-root-module-path
      (lambda (_%source-root2376%_ _%module2377%_)
        (if (or (string=? _%source-root2376%_ '"")
                (string=? _%source-root2376%_ '"."))
            _%module2377%_
            (string-append _%source-root2376%_ '"/" _%module2377%_))))
    (define asp-gerbil-scheme/src/build-api/source-discovery#source-root-project-modules
      (lambda (_%root2365%_
               _%source-root2366%_
               _%exclude2367%_
               _%exclude-dirs2368%_)
        (let ((_%source-path2370%_
               (path-expand _%source-root2366%_ _%root2365%_)))
          (if (file-exists? _%source-path2370%_)
              (map (lambda (_%g23712373%_)
                     (asp-gerbil-scheme/src/build-api/source-discovery#source-root-module-path
                      _%source-root2366%_
                      _%g23712373%_))
                   (asp-gerbil-scheme/src/build-api/source-discovery#upstream-project-modules
                    _%source-path2370%_
                    _%exclude2367%_
                    _%exclude-dirs2368%_))
              '()))))
    (define asp-gerbil-scheme/src/build-api/source-discovery#project-modules/ignore-filter
      (lambda (_%root2361%_ _%modules2362%_ _%respect-gitignore?2363%_)
        (if (and _%respect-gitignore?2363%_
                 (asp-gerbil-scheme/src/build-api/source-discovery#git-project?
                  _%root2361%_))
            (asp-gerbil-scheme/src/build-api/source-discovery#without-ignored-modules
             _%modules2362%_
             (asp-gerbil-scheme/src/build-api/source-discovery#git-ignored-modules
              _%root2361%_
              _%modules2362%_))
            _%modules2362%_)))
    (define asp-gerbil-scheme/src/build-api/source-discovery#all-gerbil-modules
      (let ((_%kw-lambda23172355%_
             (let ((_%kw-lambda-main23182348%_
                    (lambda (_%@@keywords2326%_
                             _%root23192327%_
                             _%exclude23202329%_
                             _%exclude-dirs23212331%_
                             _%default-project-excludes?23222333%_
                             _%respect-gitignore?23232335%_)
                      (let* ((_%root2338%_
                              (if (eq? _%root23192327%_ absent-value)
                                  '"."
                                  _%root23192327%_))
                             (_%exclude2340%_
                              (if (eq? _%exclude23202329%_ absent-value)
                                  asp-gerbil-scheme/src/build-api/source-discovery#+default-excluded-module-files+
                                  _%exclude23202329%_))
                             (_%exclude-dirs2342%_
                              (if (eq? _%exclude-dirs23212331%_ absent-value)
                                  '()
                                  _%exclude-dirs23212331%_))
                             (_%default-project-excludes?2344%_
                              (if (eq? _%default-project-excludes?23222333%_
                                       absent-value)
                                  '#t
                                  _%default-project-excludes?23222333%_))
                             (_%respect-gitignore?2346%_
                              (if (eq? _%respect-gitignore?23232335%_
                                       absent-value)
                                  '#t
                                  _%respect-gitignore?23232335%_)))
                        (asp-gerbil-scheme/src/build-api/source-discovery#all-gerbil-modules/config
                         _%root2338%_
                         _%exclude2340%_
                         _%exclude-dirs2342%_
                         _%default-project-excludes?2344%_
                         _%respect-gitignore?2346%_)))))
               (lambda (_%@@keywords2351%_ . _%args2352%_)
                 (apply _%kw-lambda-main23182348%_
                        _%@@keywords2351%_
                        (symbolic-table-ref
                         _%@@keywords2351%_
                         'root:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords2351%_
                         'exclude:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords2351%_
                         'exclude-dirs:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords2351%_
                         'default-project-excludes?:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords2351%_
                         'respect-gitignore?:
                         absent-value)
                        _%args2352%_)))))
        (lambda _%args23242358%_
          (apply keyword-dispatch
                 '#(#f
                    root:
                    exclude-dirs:
                    #f
                    #f
                    respect-gitignore?:
                    #f
                    exclude:
                    #f
                    default-project-excludes?:
                    #f
                    #f
                    #f
                    #f
                    #f
                    #f
                    #f
                    #f
                    #f
                    #f
                    #f
                    #f)
                 _%kw-lambda23172355%_
                 _%args23242358%_))))
    (define asp-gerbil-scheme/src/build-api/source-discovery#all-gerbil-modules/config
      (lambda (_%root2308%_
               _%exclude2309%_
               _%exclude-dirs2310%_
               _%default-project-excludes?2311%_
               _%respect-gitignore?2312%_)
        (let* ((_%effective-exclude-dirs2314%_
                (asp-gerbil-scheme/src/build-api/source-discovery#effective-project-exclude-directories
                 _%default-project-excludes?2311%_
                 _%exclude-dirs2310%_))
               (_%modules2316%_
                (asp-gerbil-scheme/src/build-api/source-discovery#upstream-project-modules
                 _%root2308%_
                 _%exclude2309%_
                 _%effective-exclude-dirs2314%_)))
          (asp-gerbil-scheme/src/build-api/source-discovery#project-modules/ignore-filter
           _%root2308%_
           _%modules2316%_
           _%respect-gitignore?2312%_))))
    (define asp-gerbil-scheme/src/build-api/source-discovery#all-gerbil-modules/roots/config
      (lambda (_%root2294%_
               _%roots2295%_
               _%exclude2296%_
               _%exclude-dirs2297%_
               _%default-project-excludes?2298%_
               _%respect-gitignore?2299%_)
        (let* ((_%effective-exclude-dirs2301%_
                (asp-gerbil-scheme/src/build-api/source-discovery#effective-project-exclude-directories
                 _%default-project-excludes?2298%_
                 _%exclude-dirs2297%_))
               (_%modules2305%_
                (std/sort#sort
                 (std/srfi/1#delete-duplicates
                  (std/srfi/1#append-map
                   (lambda (_%source-root2303%_)
                     (asp-gerbil-scheme/src/build-api/source-discovery#source-root-project-modules
                      _%root2294%_
                      _%source-root2303%_
                      _%exclude2296%_
                      _%effective-exclude-dirs2301%_))
                   _%roots2295%_))
                 string<?)))
          (asp-gerbil-scheme/src/build-api/source-discovery#project-modules/ignore-filter
           _%root2294%_
           _%modules2305%_
           _%respect-gitignore?2299%_))))))
