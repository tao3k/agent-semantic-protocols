(declare (block) (standard-bindings) (extended-bindings))
(begin
  (define asp-gerbil-scheme/src/build-api/builder-profile::timestamp
    1789096515)
  (begin
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-prototype
      (clan/poo/object#make-object
       'supers:
       '()
       'slots:
       (list (cons 'name
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%self3072%_)
                      (let* ((_%object3141%_ _%self3072%_)
                             (_%object3212%_ _%object3141%_)
                             (_%object3283%_ _%object3212%_)
                             (_%object3354%_ _%object3283%_)
                             (_%object3425%_ _%object3354%_)
                             (_%object3496%_ _%object3425%_)
                             (_%object3567%_ _%object3496%_))
                        'builder))))
             (cons 'native-profile
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%self3638%_)
                      (let* ((_%object3706%_ _%self3638%_)
                             (_%object3777%_ _%object3706%_)
                             (_%object3848%_ _%object3777%_)
                             (_%object3919%_ _%object3848%_)
                             (_%object3990%_ _%object3919%_)
                             (_%object4061%_ _%object3990%_)
                             (_%object4132%_ _%object4061%_))
                        'development))))
             (cons 'build-environment-profile
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%self4203%_)
                      (let* ((_%object4271%_ _%self4203%_)
                             (_%object4342%_ _%object4271%_)
                             (_%object4413%_ _%object4342%_)
                             (_%object4484%_ _%object4413%_)
                             (_%object4555%_ _%object4484%_)
                             (_%object4626%_ _%object4555%_)
                             (_%object4697%_ _%object4626%_))
                        asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-host-build-environment-profile))))
             (cons 'profiles
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%self4768%_)
                      (let* ((_%object4836%_ _%self4768%_)
                             (_%object4907%_ _%object4836%_)
                             (_%object4978%_ _%object4907%_)
                             (_%object5049%_ _%object4978%_)
                             (_%object5120%_ _%object5049%_)
                             (_%object5191%_ _%object5120%_)
                             (_%object5262%_ _%object5191%_))
                        (cons 'asp-quality '())))))
             (cons 'exclude-directories
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%self5333%_)
                      (let* ((_%object5401%_ _%self5333%_)
                             (_%object5472%_ _%object5401%_)
                             (_%object5543%_ _%object5472%_)
                             (_%object5614%_ _%object5543%_)
                             (_%object5685%_ _%object5614%_)
                             (_%object5756%_ _%object5685%_)
                             (_%object5827%_ _%object5756%_))
                        (cons '"scenarios" (cons '"snapshots" '()))))))
             (cons 'test-roots
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%self5898%_)
                      (let* ((_%object5966%_ _%self5898%_)
                             (_%object6037%_ _%object5966%_)
                             (_%object6108%_ _%object6037%_)
                             (_%object6179%_ _%object6108%_)
                             (_%object6250%_ _%object6179%_)
                             (_%object6321%_ _%object6250%_)
                             (_%object6392%_ _%object6321%_))
                        (cons '"t" (cons '"test" '()))))))
             (cons 'gitignore?
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%self6463%_)
                      (let* ((_%object6531%_ _%self6463%_)
                             (_%object6602%_ _%object6531%_)
                             (_%object6673%_ _%object6602%_)
                             (_%object6744%_ _%object6673%_)
                             (_%object6815%_ _%object6744%_)
                             (_%object6886%_ _%object6815%_)
                             (_%object6957%_ _%object6886%_))
                        '#t))))
             (cons 'default-project-excludes?
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%self7028%_)
                      (let* ((_%object7096%_ _%self7028%_)
                             (_%object7167%_ _%object7096%_)
                             (_%object7238%_ _%object7167%_)
                             (_%object7309%_ _%object7238%_)
                             (_%object7380%_ _%object7309%_)
                             (_%object7451%_ _%object7380%_)
                             (_%object7522%_ _%object7451%_))
                        '#t)))))
       'defaults:
       (list)))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-native-profile
      (lambda (_%value3070%_)
        (asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
         _%value3070%_
         'native-profile)))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-build-environment
      (lambda (_%value3068%_)
        (asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
         _%value3068%_
         'build-environment-profile)))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-profiles
      (lambda (_%value3066%_)
        (asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
         _%value3066%_
         'profiles)))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-exclude-directories
      (lambda (_%value3064%_)
        (asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
         _%value3064%_
         'exclude-directories)))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-test-roots
      (lambda (_%value3062%_)
        (asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
         _%value3062%_
         'test-roots)))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-gitignore?
      (lambda (_%value3060%_)
        (asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
         _%value3060%_
         'gitignore?)))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-default-project-excludes?
      (lambda (_%value3057%_)
        (asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
         _%value3057%_
         'default-project-excludes?)))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-development-builder-profile
      (clan/poo/object#make-object
       'supers:
       asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-prototype
       'slots:
       (list (cons 'name
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%@2779%_)
                      (let ((_%object2847%_ _%@2779%_)) 'development))))
             (cons 'native-profile
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%@2918%_)
                      (let ((_%object2986%_ _%@2918%_)) 'development)))))
       'defaults:
       (list)))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-production-builder-profile
      (clan/poo/object#make-object
       'supers:
       asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-prototype
       'slots:
       (list (cons 'name
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%@2499%_)
                      (let ((_%object2569%_ _%@2499%_)) 'production))))
             (cons 'native-profile
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%@2640%_)
                      (let ((_%object2708%_ _%@2640%_)) 'production)))))
       'defaults:
       (list)))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-apply-build-environment!
      (lambda (_%profile2497%_)
        (asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-apply-build-environment-profile!
         (asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-build-environment
          _%profile2497%_))))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-module-under-root?
      (lambda (_%module2485%_ _%root2486%_)
        (let ((_%$e2488%_ (string=? _%root2486%_ '"")))
          (if _%$e2488%_
              _%$e2488%_
              (let ((_%$e2491%_ (string=? _%root2486%_ '".")))
                (if _%$e2491%_
                    _%$e2491%_
                    (let ((_%$e2494%_ (string=? _%module2485%_ _%root2486%_)))
                      (if _%$e2494%_
                          _%$e2494%_
                          (std/srfi/13#string-prefix?
                           (string-append _%root2486%_ '"/")
                           _%module2485%_)))))))))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-modules
      (let ((_%kw-lambda24452479%_
             (let ((_%kw-lambda-main24462472%_
                    (lambda (_%@@keywords2453%_
                             _%root24472454%_
                             _%roots24482456%_
                             _%exclude24492458%_
                             _%exclude-dirs24502460%_
                             _%profile2462%_)
                      (let* ((_%root2464%_
                              (if (eq? _%root24472454%_ absent-value)
                                  '"."
                                  _%root24472454%_))
                             (_%roots2466%_
                              (if (eq? _%roots24482456%_ absent-value)
                                  (cons '"." '())
                                  _%roots24482456%_))
                             (_%exclude2468%_
                              (if (eq? _%exclude24492458%_ absent-value)
                                  asp-gerbil-scheme/src/build-api/source-discovery#+default-excluded-module-files+
                                  _%exclude24492458%_))
                             (_%exclude-dirs2470%_
                              (if (eq? _%exclude-dirs24502460%_ absent-value)
                                  (asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-exclude-directories
                                   _%profile2462%_)
                                  _%exclude-dirs24502460%_)))
                        (asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-modules/config
                         _%profile2462%_
                         _%root2464%_
                         _%roots2466%_
                         _%exclude2468%_
                         _%exclude-dirs2470%_)))))
               (lambda (_%@@keywords2475%_ . _%args2476%_)
                 (apply _%kw-lambda-main24462472%_
                        _%@@keywords2475%_
                        (symbolic-table-ref
                         _%@@keywords2475%_
                         'root:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords2475%_
                         'roots:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords2475%_
                         'exclude:
                         absent-value)
                        (symbolic-table-ref
                         _%@@keywords2475%_
                         'exclude-dirs:
                         absent-value)
                        _%args2476%_)))))
        (lambda _%args24512482%_
          (apply keyword-dispatch
                 '#(#f #f roots: root: exclude-dirs: exclude:)
                 _%kw-lambda24452479%_
                 _%args24512482%_))))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-modules/root-roots
      (lambda (_%profile2442%_ _%root2443%_ _%roots2444%_)
        (asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-modules/config
         _%profile2442%_
         _%root2443%_
         _%roots2444%_
         asp-gerbil-scheme/src/build-api/source-discovery#+default-excluded-module-files+
         (asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-exclude-directories
          _%profile2442%_))))
    (define asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-modules/config
      (lambda (_%profile2436%_
               _%root2437%_
               _%roots2438%_
               _%exclude2439%_
               _%exclude-dirs2440%_)
        (asp-gerbil-scheme/src/build-api/source-discovery#all-gerbil-modules/roots/config
         _%root2437%_
         _%roots2438%_
         _%exclude2439%_
         _%exclude-dirs2440%_
         (asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-default-project-excludes?
          _%profile2436%_)
         (asp-gerbil-scheme/src/build-api/builder-profile#asp-gerbil-scheme-builder-profile-gitignore?
          _%profile2436%_))))))
