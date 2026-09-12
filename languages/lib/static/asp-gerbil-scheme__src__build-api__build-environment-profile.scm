(declare (block) (standard-bindings) (extended-bindings))
(begin
  (define asp-gerbil-scheme/src/build-api/build-environment-profile::timestamp
    1789096507)
  (begin
    (define asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-build-environment-profile-prototype
      (clan/poo/object#make-object
       'supers:
       '()
       'slots:
       (list (cons 'name
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%self1943%_)
                      (let ((_%object2012%_ _%self1943%_)) 'portable))))
             (cons 'bindings
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%self2083%_)
                      (let ((_%object2151%_ _%self2083%_)) '())))))
       'defaults:
       (list)))
    (define asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-build-environment-profile-name
      (lambda (_%value1941%_)
        (asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
         _%value1941%_
         'name)))
    (define asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-build-environment-profile-bindings
      (lambda (_%value1938%_)
        (asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
         _%value1938%_
         'bindings)))
    (define asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-portable-build-environment-profile
      (clan/poo/object#make-object
       'supers:
       asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-build-environment-profile-prototype
       'slots:
       (list)
       'defaults:
       (list)))
    (define asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-macos-build-environment-profile
      (clan/poo/object#make-object
       'supers:
       asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-build-environment-profile-prototype
       'slots:
       (list (cons 'name
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%@1658%_)
                      (let ((_%object1728%_ _%@1658%_)) 'macos-native))))
             (cons 'bindings
                   (clan/poo/object#make-$self-slot-spec
                    (lambda (_%@1799%_)
                      (let ((_%object1867%_ _%@1799%_))
                        '(("SDKROOT" . #f) ("DEVELOPER_DIR" . #f)))))))
       'defaults:
       (list)))
    (define asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-host-build-environment-profile
      asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-macos-build-environment-profile)
    (define asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-apply-build-environment-profile!
      (lambda (_%profile1651%_)
        (for-each
         (lambda (_%binding1653%_)
           (let ((_%name1655%_ (car _%binding1653%_))
                 (_%value1656%_ (cdr _%binding1653%_)))
             (if _%value1656%_
                 (setenv _%name1655%_ _%value1656%_)
                 (setenv _%name1655%_))))
         (asp-gerbil-scheme/src/build-api/build-environment-profile#asp-gerbil-scheme-build-environment-profile-bindings
          _%profile1651%_))))))
