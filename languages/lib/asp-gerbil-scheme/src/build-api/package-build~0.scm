(declare (block) (standard-bindings) (extended-bindings))
(begin
  (define asp-gerbil-scheme/src/build-api/package-build::timestamp 1789096527)
  (begin
    (define asp-gerbil-scheme/src/build-api/package-build#package-root '#f)
    (define asp-gerbil-scheme/src/build-api/package-build#package-build-non-empty-string?
      (lambda (_%value247%_)
        (if (string? _%value247%_) (> (string-length _%value247%_) '0) '#f)))
    (define asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-build-package-name
      (lambda (_%root228%_)
        (let* ((_%package-file230%_ (path-expand '"gerbil.pkg" _%root228%_))
               (_%plist235%_
                (with-catch
                 (lambda (_%_232%_) '#f)
                 (lambda () (call-with-input-file _%package-file230%_ read)))))
          (let _%loop238%_ ((_%rest240%_ _%plist235%_))
            (if (and (pair? _%rest240%_) (pair? (cdr _%rest240%_)))
                (if (eq? (car _%rest240%_) 'package:)
                    (let ((_%name242%_ (cadr _%rest240%_)))
                      (if (symbol? _%name242%_)
                          (symbol->string _%name242%_)
                          (if (string? _%name242%_) _%name242%_ '#f)))
                    (_%loop238%_ (cdr _%rest240%_)))
                '#f)))))
    (define asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-build-active-gerbil-path
      (lambda (_%root224%_)
        (path-expand
         (let ((_%path226%_ (getenv '"GERBIL_PATH" '#f)))
           (if (asp-gerbil-scheme/src/build-api/package-build#package-build-non-empty-string?
                _%path226%_)
               _%path226%_
               (gerbil-home))))))
    (define asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-build-active-gerbil-lib-path
      (lambda (_%root222%_)
        (path-expand
         '"lib"
         (asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-build-active-gerbil-path
          _%root222%_))))
    (define asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-configure-build-root!
      (lambda (_%root218%_)
        (let ((_%active-gerbil-path220%_
               (asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-build-active-gerbil-path
                _%root218%_)))
          (set! asp-gerbil-scheme/src/build-api/package-build#package-root
                (path-normalize _%root218%_))
          (current-directory
           asp-gerbil-scheme/src/build-api/package-build#package-root)
          (add-load-path! (path-expand '"lib" _%active-gerbil-path220%_)))))
    (define asp-gerbil-scheme/src/build-api/package-build#ensure-package-build-root!
      (lambda ()
        (if asp-gerbil-scheme/src/build-api/package-build#package-root
            '#!void
            (asp-gerbil-scheme/src/build-api/package-build#asp-gerbil-scheme-package-configure-build-root!
             (current-directory)))))))
