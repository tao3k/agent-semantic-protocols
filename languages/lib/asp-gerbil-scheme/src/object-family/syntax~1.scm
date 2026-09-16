(declare (block) (standard-bindings) (extended-bindings) (inlining-limit 200))
(begin
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48196_|
    (##structure
     gx#syntax-quote::t
     'accessors
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48197_|
    (##structure
     gx#syntax-quote::t
     'required
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48200_|
    (##structure
     gx#syntax-quote::t
     'optional
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48203_|
    (##structure
     gx#syntax-quote::t
     'prototype
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48206_|
    (##structure
     gx#syntax-quote::t
     'accessors
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48207_|
    (##structure
     gx#syntax-quote::t
     'required
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48210_|
    (##structure
     gx#syntax-quote::t
     'optional
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48213_|
    (##structure
     gx#syntax-quote::t
     'prototype
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48216_|
    (##structure
     gx#syntax-quote::t
     'constructor
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48219_|
    (##structure
     gx#syntax-quote::t
     'accessors
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48220_|
    (##structure
     gx#syntax-quote::t
     'required
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[1]#_g48223_|
    (##structure
     gx#syntax-quote::t
     'optional
     #f
     (gx#current-expander-context)
     '()))
  (define |asp-gerbil-scheme/src/object-family/syntax[:0:]#defpoo-object-family|
    (lambda (_%$stx260%_)
      (let* ((_%g266540%_
              (lambda (_%g267536%_)
                (gx#raise-syntax-error
                 '#f
                 '"Bad syntax; invalid match target"
                 _%g267536%_)))
             (_%g265821%_
              (lambda (_%g267544%_)
                (if (gx#stx-pair? _%g267544%_)
                    (let ((_%e472547%_ (gx#syntax-e _%g267544%_)))
                      (let ((_%hd473551%_
                             (let () (declare (not safe)) (##car _%e472547%_)))
                            (_%tl474554%_
                             (let ()
                               (declare (not safe))
                               (##cdr _%e472547%_))))
                        (if (gx#stx-pair? _%tl474554%_)
                            (let ((_%e475557%_ (gx#syntax-e _%tl474554%_)))
                              (let ((_%hd476561%_
                                     (let ()
                                       (declare (not safe))
                                       (##car _%e475557%_)))
                                    (_%tl477564%_
                                     (let ()
                                       (declare (not safe))
                                       (##cdr _%e475557%_))))
                                (if (gx#stx-pair? _%hd476561%_)
                                    (let ((_%e478567%_
                                           (gx#syntax-e _%hd476561%_)))
                                      (let ((_%hd479571%_
                                             (let ()
                                               (declare (not safe))
                                               (##car _%e478567%_)))
                                            (_%tl480574%_
                                             (let ()
                                               (declare (not safe))
                                               (##cdr _%e478567%_))))
                                        (if (gx#identifier? _%hd479571%_)
                                            (if (gx#free-identifier=?
                                                 |asp-gerbil-scheme/src/object-family/syntax[1]#_g48196_|
                                                 _%hd479571%_)
                                                (if (gx#stx-pair? _%tl480574%_)
                                                    (let ((_%e481577%_
                                                           (gx#syntax-e
                                                            _%tl480574%_)))
                                                      (let ((_%hd482581%_
                                                             (let ()
                                                               (declare
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (not safe))
                       (##car _%e481577%_)))
                    (_%tl483584%_
                     (let () (declare (not safe)) (##cdr _%e481577%_))))
                (if (gx#stx-pair? _%tl483584%_)
                    (let ((_%e484587%_ (gx#syntax-e _%tl483584%_)))
                      (let ((_%hd485591%_
                             (let () (declare (not safe)) (##car _%e484587%_)))
                            (_%tl486594%_
                             (let ()
                               (declare (not safe))
                               (##cdr _%e484587%_))))
                        (if (gx#stx-pair? _%hd485591%_)
                            (let ((_%e487597%_ (gx#syntax-e _%hd485591%_)))
                              (let ((_%hd488601%_
                                     (let ()
                                       (declare (not safe))
                                       (##car _%e487597%_)))
                                    (_%tl489604%_
                                     (let ()
                                       (declare (not safe))
                                       (##cdr _%e487597%_))))
                                (if (gx#identifier? _%hd488601%_)
                                    (if (gx#free-identifier=?
                                         |asp-gerbil-scheme/src/object-family/syntax[1]#_g48197_|
                                         _%hd488601%_)
                                        (if (gx#stx-pair/null? _%tl489604%_)
                                            (let ((_g48198_
                                                   (gx#syntax-split-splice
                                                    _%tl489604%_
                                                    '0)))
                                              (begin
                                                (let ((_g48199_
                                                       (let ()
                                                         (declare (not safe))
                                                         (if (##values?
                                                              _g48198_)
                                                             (##values-length
                                                              _g48198_)
                                                             1))))
                                                  (if (not (let ()
                                                             (declare
                                                               (not safe))
                                                             (##fx= _g48199_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                            2)))
              (error "Context expects 2 values" _g48199_)))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                (let ((_%target490607%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##values-ref
                                                          _g48198_
                                                          0)))
                                                      (_%tl492610%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##values-ref
                                                          _g48198_
                                                          1))))
                                                  (if (gx#stx-null?
                                                       _%tl492610%_)
                                                      (letrec ((_%loop493613%_
                                                                (lambda (_%hd491617%_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                                 _%required-accessor-slot497620%_
                                 _%required-accessor-name498622%_)
                          (if (gx#stx-pair? _%hd491617%_)
                              (let ((_%e494625%_ (gx#syntax-e _%hd491617%_)))
                                (let ((_%lp-hd495629%_
                                       (let ()
                                         (declare (not safe))
                                         (##car _%e494625%_)))
                                      (_%lp-tl496632%_
                                       (let ()
                                         (declare (not safe))
                                         (##cdr _%e494625%_))))
                                  (if (gx#stx-pair? _%lp-hd495629%_)
                                      (let ((_%e501635%_
                                             (gx#syntax-e _%lp-hd495629%_)))
                                        (let ((_%hd502639%_
                                               (let ()
                                                 (declare (not safe))
                                                 (##car _%e501635%_)))
                                              (_%tl503642%_
                                               (let ()
                                                 (declare (not safe))
                                                 (##cdr _%e501635%_))))
                                          (if (gx#stx-pair? _%tl503642%_)
                                              (let ((_%e504645%_
                                                     (gx#syntax-e
                                                      _%tl503642%_)))
                                                (let ((_%hd505649%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##car _%e504645%_)))
                                                      (_%tl506652%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##cdr _%e504645%_))))
                                                  (if (gx#stx-null?
                                                       _%tl506652%_)
                                                      (_%loop493613%_
                                                       _%lp-tl496632%_
                                                       (cons _%hd505649%_
                                                             _%required-accessor-slot497620%_)
                                                       (cons _%hd502639%_
                                                             _%required-accessor-name498622%_))
                                                      (_%g266540%_
                                                       _%g267544%_))))
                                              (_%g266540%_ _%g267544%_))))
                                      (_%g266540%_ _%g267544%_))))
                              (let ((_%required-accessor-slot499655%_
                                     (reverse _%required-accessor-slot497620%_))
                                    (_%required-accessor-name500658%_
                                     (reverse _%required-accessor-name498622%_)))
                                (if (gx#stx-pair? _%tl486594%_)
                                    (let ((_%e507661%_
                                           (gx#syntax-e _%tl486594%_)))
                                      (let ((_%hd508665%_
                                             (let ()
                                               (declare (not safe))
                                               (##car _%e507661%_)))
                                            (_%tl509668%_
                                             (let ()
                                               (declare (not safe))
                                               (##cdr _%e507661%_))))
                                        (if (gx#stx-pair? _%hd508665%_)
                                            (let ((_%e510671%_
                                                   (gx#syntax-e _%hd508665%_)))
                                              (let ((_%hd511675%_
                                                     (let ()
                                                       (declare (not safe))
                                                       (##car _%e510671%_)))
                                                    (_%tl512678%_
                                                     (let ()
                                                       (declare (not safe))
                                                       (##cdr _%e510671%_))))
                                                (if (gx#identifier?
                                                     _%hd511675%_)
                                                    (if (gx#free-identifier=?
                                                         |asp-gerbil-scheme/src/object-family/syntax[1]#_g48200_|
                                                         _%hd511675%_)
                                                        (if (gx#stx-pair/null?
                                                             _%tl512678%_)
                                                            (let ((_g48201_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                           (gx#syntax-split-splice _%tl512678%_ '0)))
                      (begin
                        (let ((_g48202_
                               (let ()
                                 (declare (not safe))
                                 (if (##values? _g48201_)
                                     (##values-length _g48201_)
                                     1))))
                          (if (not (let ()
                                     (declare (not safe))
                                     (##fx= _g48202_ 2)))
                              (error "Context expects 2 values" _g48202_)))
                        (let ((_%target513681%_
                               (let ()
                                 (declare (not safe))
                                 (##values-ref _g48201_ 0)))
                              (_%tl515684%_
                               (let ()
                                 (declare (not safe))
                                 (##values-ref _g48201_ 1))))
                          (if (gx#stx-null? _%tl515684%_)
                              (letrec ((_%loop516687%_
                                        (lambda (_%hd514691%_
                                                 _%default-value520694%_
                                                 _%optional-accessor-slot521696%_
                                                 _%optional-accessor-name522698%_)
                                          (if (gx#stx-pair? _%hd514691%_)
                                              (let ((_%e517701%_
                                                     (gx#syntax-e
                                                      _%hd514691%_)))
                                                (let ((_%lp-hd518705%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##car _%e517701%_)))
                                                      (_%lp-tl519708%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##cdr _%e517701%_))))
                                                  (if (gx#stx-pair?
                                                       _%lp-hd518705%_)
                                                      (let ((_%e526711%_
                                                             (gx#syntax-e
                                                              _%lp-hd518705%_)))
                                                        (let ((_%hd527715%_
                                                               (let ()
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (declare (not safe))
                         (##car _%e526711%_)))
                      (_%tl528718%_
                       (let () (declare (not safe)) (##cdr _%e526711%_))))
                  (if (gx#stx-pair? _%tl528718%_)
                      (let ((_%e529721%_ (gx#syntax-e _%tl528718%_)))
                        (let ((_%hd530725%_
                               (let ()
                                 (declare (not safe))
                                 (##car _%e529721%_)))
                              (_%tl531728%_
                               (let ()
                                 (declare (not safe))
                                 (##cdr _%e529721%_))))
                          (if (gx#stx-pair? _%tl531728%_)
                              (let ((_%e532731%_ (gx#syntax-e _%tl531728%_)))
                                (let ((_%hd533735%_
                                       (let ()
                                         (declare (not safe))
                                         (##car _%e532731%_)))
                                      (_%tl534738%_
                                       (let ()
                                         (declare (not safe))
                                         (##cdr _%e532731%_))))
                                  (if (gx#stx-null? _%tl534738%_)
                                      (_%loop516687%_
                                       _%lp-tl519708%_
                                       (cons _%hd533735%_
                                             _%default-value520694%_)
                                       (cons _%hd530725%_
                                             _%optional-accessor-slot521696%_)
                                       (cons _%hd527715%_
                                             _%optional-accessor-name522698%_))
                                      (_%g266540%_ _%g267544%_))))
                              (_%g266540%_ _%g267544%_))))
                      (_%g266540%_ _%g267544%_))))
              (_%g266540%_ _%g267544%_))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                              (let ((_%default-value523741%_
                                                     (reverse _%default-value520694%_))
                                                    (_%optional-accessor-slot524744%_
                                                     (reverse _%optional-accessor-slot521696%_))
                                                    (_%optional-accessor-name525746%_
                                                     (reverse _%optional-accessor-name522698%_)))
                                                (if (gx#stx-null? _%tl509668%_)
                                                    (if (gx#stx-null?
                                                         _%tl477564%_)
                                                        ((lambda (_%L749%_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                          _%L751%_
                          _%L752%_
                          _%L753%_
                          _%L754%_
                          _%L755%_)
                   (cons (gx#datum->syntax '#f 'begin)
                         (begin
                           (gx#syntax-check-splice-targets _%L753%_ _%L754%_)
                           (foldr (lambda (_%g795803%_ _%g796806%_ _%g797808%_)
                                    (cons (cons (gx#datum->syntax '#f 'def)
                                                (cons (cons _%g796806%_
                                                            (cons (gx#datum->syntax
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                           '#f
                           'value)
                          '()))
              (cons (cons _%L755%_
                          (cons (gx#datum->syntax '#f 'value)
                                (cons (cons (gx#datum->syntax '#f 'quote)
                                            (cons _%g795803%_ '()))
                                      '())))
                    '())))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                          _%g797808%_))
                                  (begin
                                    (gx#syntax-check-splice-targets
                                     _%L749%_
                                     _%L751%_
                                     _%L752%_)
                                    (foldr (lambda (_%g798811%_
                                                    _%g799814%_
                                                    _%g800816%_
                                                    _%g801818%_)
                                             (cons (cons (gx#datum->syntax
                                                          '#f
                                                          'def)
                                                         (cons (cons _%g800816%_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                             (cons (gx#datum->syntax '#f 'value) '()))
                       (cons (cons _%L755%_
                                   (cons (gx#datum->syntax '#f 'value)
                                         (cons (cons (gx#datum->syntax
                                                      '#f
                                                      'quote)
                                                     (cons _%g799814%_ '()))
                                               (cons _%g798811%_ '()))))
                             '())))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                   _%g801818%_))
                                           '()
                                           _%L749%_
                                           _%L751%_
                                           _%L752%_))
                                  _%L753%_
                                  _%L754%_))))
                 _%default-value523741%_
                 _%optional-accessor-slot524744%_
                 _%optional-accessor-name525746%_
                 _%required-accessor-slot499655%_
                 _%required-accessor-name500658%_
                 _%hd482581%_)
                (_%g266540%_ _%g267544%_))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g266540%_
                                                     _%g267544%_)))))))
                                (_%loop516687%_ _%target513681%_ '() '() '()))
                              (_%g266540%_ _%g267544%_)))))
                    (_%g266540%_ _%g267544%_))
                (_%g266540%_ _%g267544%_))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g266540%_
                                                     _%g267544%_))))
                                            (_%g266540%_ _%g267544%_))))
                                    (_%g266540%_ _%g267544%_)))))))
                (_%loop493613%_ _%target490607%_ '() '()))
              (_%g266540%_ _%g267544%_)))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                            (_%g266540%_ _%g267544%_))
                                        (_%g266540%_ _%g267544%_))
                                    (_%g266540%_ _%g267544%_))))
                            (_%g266540%_ _%g267544%_))))
                    (_%g266540%_ _%g267544%_))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g266540%_ _%g267544%_))
                                                (_%g266540%_ _%g267544%_))
                                            (_%g266540%_ _%g267544%_))))
                                    (_%g266540%_ _%g267544%_))))
                            (_%g266540%_ _%g267544%_))))
                    (_%g266540%_ _%g267544%_))))
             (_%g2641180%_
              (lambda (_%g267825%_)
                (if (gx#stx-pair? _%g267825%_)
                    (let ((_%e385828%_ (gx#syntax-e _%g267825%_)))
                      (let ((_%hd386832%_
                             (let () (declare (not safe)) (##car _%e385828%_)))
                            (_%tl387835%_
                             (let ()
                               (declare (not safe))
                               (##cdr _%e385828%_))))
                        (if (gx#stx-pair? _%tl387835%_)
                            (let ((_%e388838%_ (gx#syntax-e _%tl387835%_)))
                              (let ((_%hd389842%_
                                     (let ()
                                       (declare (not safe))
                                       (##car _%e388838%_)))
                                    (_%tl390845%_
                                     (let ()
                                       (declare (not safe))
                                       (##cdr _%e388838%_))))
                                (if (gx#stx-pair? _%hd389842%_)
                                    (let ((_%e391848%_
                                           (gx#syntax-e _%hd389842%_)))
                                      (let ((_%hd392852%_
                                             (let ()
                                               (declare (not safe))
                                               (##car _%e391848%_)))
                                            (_%tl393855%_
                                             (let ()
                                               (declare (not safe))
                                               (##cdr _%e391848%_))))
                                        (if (gx#identifier? _%hd392852%_)
                                            (if (gx#free-identifier=?
                                                 |asp-gerbil-scheme/src/object-family/syntax[1]#_g48203_|
                                                 _%hd392852%_)
                                                (if (gx#stx-pair? _%tl393855%_)
                                                    (let ((_%e394858%_
                                                           (gx#syntax-e
                                                            _%tl393855%_)))
                                                      (let ((_%hd395862%_
                                                             (let ()
                                                               (declare
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (not safe))
                       (##car _%e394858%_)))
                    (_%tl396865%_
                     (let () (declare (not safe)) (##cdr _%e394858%_))))
                (if (gx#stx-pair/null? _%tl396865%_)
                    (let ((_g48204_ (gx#syntax-split-splice _%tl396865%_ '0)))
                      (begin
                        (let ((_g48205_
                               (let ()
                                 (declare (not safe))
                                 (if (##values? _g48204_)
                                     (##values-length _g48204_)
                                     1))))
                          (if (not (let ()
                                     (declare (not safe))
                                     (##fx= _g48205_ 2)))
                              (error "Context expects 2 values" _g48205_)))
                        (let ((_%target397868%_
                               (let ()
                                 (declare (not safe))
                                 (##values-ref _g48204_ 0)))
                              (_%tl399871%_
                               (let ()
                                 (declare (not safe))
                                 (##values-ref _g48204_ 1))))
                          (if (gx#stx-null? _%tl399871%_)
                              (letrec ((_%loop400874%_
                                        (lambda (_%hd398878%_
                                                 _%prototype-slot404881%_)
                                          (if (gx#stx-pair? _%hd398878%_)
                                              (let ((_%e401884%_
                                                     (gx#syntax-e
                                                      _%hd398878%_)))
                                                (let ((_%lp-hd402888%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##car _%e401884%_)))
                                                      (_%lp-tl403891%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##cdr _%e401884%_))))
                                                  (_%loop400874%_
                                                   _%lp-tl403891%_
                                                   (cons _%lp-hd402888%_
                                                         _%prototype-slot404881%_))))
                                              (let ((_%prototype-slot405894%_
                                                     (reverse _%prototype-slot404881%_)))
                                                (if (gx#stx-pair? _%tl390845%_)
                                                    (let ((_%e406898%_
                                                           (gx#syntax-e
                                                            _%tl390845%_)))
                                                      (let ((_%hd407902%_
                                                             (let ()
                                                               (declare
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (not safe))
                       (##car _%e406898%_)))
                    (_%tl408905%_
                     (let () (declare (not safe)) (##cdr _%e406898%_))))
                (if (gx#stx-pair? _%hd407902%_)
                    (let ((_%e409908%_ (gx#syntax-e _%hd407902%_)))
                      (let ((_%hd410912%_
                             (let () (declare (not safe)) (##car _%e409908%_)))
                            (_%tl411915%_
                             (let ()
                               (declare (not safe))
                               (##cdr _%e409908%_))))
                        (if (gx#identifier? _%hd410912%_)
                            (if (gx#free-identifier=?
                                 |asp-gerbil-scheme/src/object-family/syntax[1]#_g48206_|
                                 _%hd410912%_)
                                (if (gx#stx-pair? _%tl411915%_)
                                    (let ((_%e412918%_
                                           (gx#syntax-e _%tl411915%_)))
                                      (let ((_%hd413922%_
                                             (let ()
                                               (declare (not safe))
                                               (##car _%e412918%_)))
                                            (_%tl414925%_
                                             (let ()
                                               (declare (not safe))
                                               (##cdr _%e412918%_))))
                                        (if (gx#stx-pair? _%tl414925%_)
                                            (let ((_%e415928%_
                                                   (gx#syntax-e _%tl414925%_)))
                                              (let ((_%hd416932%_
                                                     (let ()
                                                       (declare (not safe))
                                                       (##car _%e415928%_)))
                                                    (_%tl417935%_
                                                     (let ()
                                                       (declare (not safe))
                                                       (##cdr _%e415928%_))))
                                                (if (gx#stx-pair? _%hd416932%_)
                                                    (let ((_%e418938%_
                                                           (gx#syntax-e
                                                            _%hd416932%_)))
                                                      (let ((_%hd419942%_
                                                             (let ()
                                                               (declare
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (not safe))
                       (##car _%e418938%_)))
                    (_%tl420945%_
                     (let () (declare (not safe)) (##cdr _%e418938%_))))
                (if (gx#identifier? _%hd419942%_)
                    (if (gx#free-identifier=?
                         |asp-gerbil-scheme/src/object-family/syntax[1]#_g48207_|
                         _%hd419942%_)
                        (if (gx#stx-pair/null? _%tl420945%_)
                            (let ((_g48208_
                                   (gx#syntax-split-splice _%tl420945%_ '0)))
                              (begin
                                (let ((_g48209_
                                       (let ()
                                         (declare (not safe))
                                         (if (##values? _g48208_)
                                             (##values-length _g48208_)
                                             1))))
                                  (if (not (let ()
                                             (declare (not safe))
                                             (##fx= _g48209_ 2)))
                                      (error "Context expects 2 values"
                                             _g48209_)))
                                (let ((_%target421948%_
                                       (let ()
                                         (declare (not safe))
                                         (##values-ref _g48208_ 0)))
                                      (_%tl423951%_
                                       (let ()
                                         (declare (not safe))
                                         (##values-ref _g48208_ 1))))
                                  (if (gx#stx-null? _%tl423951%_)
                                      (letrec ((_%loop424954%_
                                                (lambda (_%hd422958%_
                                                         _%required-accessor-slot428961%_
                                                         _%required-accessor-name429963%_)
                                                  (if (gx#stx-pair?
                                                       _%hd422958%_)
                                                      (let ((_%e425966%_
                                                             (gx#syntax-e
                                                              _%hd422958%_)))
                                                        (let ((_%lp-hd426970%_
                                                               (let ()
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (declare (not safe))
                         (##car _%e425966%_)))
                      (_%lp-tl427973%_
                       (let () (declare (not safe)) (##cdr _%e425966%_))))
                  (if (gx#stx-pair? _%lp-hd426970%_)
                      (let ((_%e432976%_ (gx#syntax-e _%lp-hd426970%_)))
                        (let ((_%hd433980%_
                               (let ()
                                 (declare (not safe))
                                 (##car _%e432976%_)))
                              (_%tl434983%_
                               (let ()
                                 (declare (not safe))
                                 (##cdr _%e432976%_))))
                          (if (gx#stx-pair? _%tl434983%_)
                              (let ((_%e435986%_ (gx#syntax-e _%tl434983%_)))
                                (let ((_%hd436990%_
                                       (let ()
                                         (declare (not safe))
                                         (##car _%e435986%_)))
                                      (_%tl437993%_
                                       (let ()
                                         (declare (not safe))
                                         (##cdr _%e435986%_))))
                                  (if (gx#stx-null? _%tl437993%_)
                                      (_%loop424954%_
                                       _%lp-tl427973%_
                                       (cons _%hd436990%_
                                             _%required-accessor-slot428961%_)
                                       (cons _%hd433980%_
                                             _%required-accessor-name429963%_))
                                      (_%g265821%_ _%g267825%_))))
                              (_%g265821%_ _%g267825%_))))
                      (_%g265821%_ _%g267825%_))))
              (let ((_%required-accessor-slot430996%_
                     (reverse _%required-accessor-slot428961%_))
                    (_%required-accessor-name431999%_
                     (reverse _%required-accessor-name429963%_)))
                (if (gx#stx-pair? _%tl417935%_)
                    (let ((_%e4381002%_ (gx#syntax-e _%tl417935%_)))
                      (let ((_%hd4391006%_
                             (let ()
                               (declare (not safe))
                               (##car _%e4381002%_)))
                            (_%tl4401009%_
                             (let ()
                               (declare (not safe))
                               (##cdr _%e4381002%_))))
                        (if (gx#stx-pair? _%hd4391006%_)
                            (let ((_%e4411012%_ (gx#syntax-e _%hd4391006%_)))
                              (let ((_%hd4421016%_
                                     (let ()
                                       (declare (not safe))
                                       (##car _%e4411012%_)))
                                    (_%tl4431019%_
                                     (let ()
                                       (declare (not safe))
                                       (##cdr _%e4411012%_))))
                                (if (gx#identifier? _%hd4421016%_)
                                    (if (gx#free-identifier=?
                                         |asp-gerbil-scheme/src/object-family/syntax[1]#_g48210_|
                                         _%hd4421016%_)
                                        (if (gx#stx-pair/null? _%tl4431019%_)
                                            (let ((_g48211_
                                                   (gx#syntax-split-splice
                                                    _%tl4431019%_
                                                    '0)))
                                              (begin
                                                (let ((_g48212_
                                                       (let ()
                                                         (declare (not safe))
                                                         (if (##values?
                                                              _g48211_)
                                                             (##values-length
                                                              _g48211_)
                                                             1))))
                                                  (if (not (let ()
                                                             (declare
                                                               (not safe))
                                                             (##fx= _g48212_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                            2)))
              (error "Context expects 2 values" _g48212_)))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                (let ((_%target4441022%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##values-ref
                                                          _g48211_
                                                          0)))
                                                      (_%tl4461025%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##values-ref
                                                          _g48211_
                                                          1))))
                                                  (if (gx#stx-null?
                                                       _%tl4461025%_)
                                                      (letrec ((_%loop4471028%_
                                                                (lambda (_%hd4451032%_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                                 _%default-value4511035%_
                                 _%optional-accessor-slot4521037%_
                                 _%optional-accessor-name4531039%_)
                          (if (gx#stx-pair? _%hd4451032%_)
                              (let ((_%e4481042%_ (gx#syntax-e _%hd4451032%_)))
                                (let ((_%lp-hd4491046%_
                                       (let ()
                                         (declare (not safe))
                                         (##car _%e4481042%_)))
                                      (_%lp-tl4501049%_
                                       (let ()
                                         (declare (not safe))
                                         (##cdr _%e4481042%_))))
                                  (if (gx#stx-pair? _%lp-hd4491046%_)
                                      (let ((_%e4571052%_
                                             (gx#syntax-e _%lp-hd4491046%_)))
                                        (let ((_%hd4581056%_
                                               (let ()
                                                 (declare (not safe))
                                                 (##car _%e4571052%_)))
                                              (_%tl4591059%_
                                               (let ()
                                                 (declare (not safe))
                                                 (##cdr _%e4571052%_))))
                                          (if (gx#stx-pair? _%tl4591059%_)
                                              (let ((_%e4601062%_
                                                     (gx#syntax-e
                                                      _%tl4591059%_)))
                                                (let ((_%hd4611066%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##car _%e4601062%_)))
                                                      (_%tl4621069%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##cdr _%e4601062%_))))
                                                  (if (gx#stx-pair?
                                                       _%tl4621069%_)
                                                      (let ((_%e4631072%_
                                                             (gx#syntax-e
                                                              _%tl4621069%_)))
                                                        (let ((_%hd4641076%_
                                                               (let ()
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (declare (not safe))
                         (##car _%e4631072%_)))
                      (_%tl4651079%_
                       (let () (declare (not safe)) (##cdr _%e4631072%_))))
                  (if (gx#stx-null? _%tl4651079%_)
                      (_%loop4471028%_
                       _%lp-tl4501049%_
                       (cons _%hd4641076%_ _%default-value4511035%_)
                       (cons _%hd4611066%_ _%optional-accessor-slot4521037%_)
                       (cons _%hd4581056%_ _%optional-accessor-name4531039%_))
                      (_%g265821%_ _%g267825%_))))
              (_%g265821%_ _%g267825%_))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                              (_%g265821%_ _%g267825%_))))
                                      (_%g265821%_ _%g267825%_))))
                              (let ((_%default-value4541082%_
                                     (reverse _%default-value4511035%_))
                                    (_%optional-accessor-slot4551085%_
                                     (reverse _%optional-accessor-slot4521037%_))
                                    (_%optional-accessor-name4561087%_
                                     (reverse _%optional-accessor-name4531039%_)))
                                (if (gx#stx-null? _%tl4401009%_)
                                    (if (gx#stx-null? _%tl408905%_)
                                        ((lambda (_%L1090%_
                                                  _%L1092%_
                                                  _%L1093%_
                                                  _%L1094%_
                                                  _%L1095%_
                                                  _%L1096%_
                                                  _%L1097%_
                                                  _%L1098%_)
                                           (cons (gx#datum->syntax '#f 'begin)
                                                 (cons (cons (gx#datum->syntax
                                                              '#f
                                                              '.def)
                                                             (cons _%L1098%_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                           (foldr (lambda (_%g11461156%_ _%g11471159%_)
                                    (cons _%g11461156%_ _%g11471159%_))
                                  '()
                                  _%L1097%_)))
               (begin
                 (gx#syntax-check-splice-targets _%L1094%_ _%L1095%_)
                 (foldr (lambda (_%g11481162%_ _%g11491165%_ _%g11501167%_)
                          (cons (cons (gx#datum->syntax '#f 'def)
                                      (cons (cons _%g11491165%_
                                                  (cons (gx#datum->syntax
                                                         '#f
                                                         'value)
                                                        '()))
                                            (cons (cons _%L1096%_
                                                        (cons (gx#datum->syntax
                                                               '#f
                                                               'value)
                                                              (cons (cons (gx#datum->syntax
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                                   '#f
                                   'quote)
                                  (cons _%g11481162%_ '()))
                            '())))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                  '())))
                                _%g11501167%_))
                        (begin
                          (gx#syntax-check-splice-targets
                           _%L1090%_
                           _%L1092%_
                           _%L1093%_)
                          (foldr (lambda (_%g11511170%_
                                          _%g11521173%_
                                          _%g11531175%_
                                          _%g11541177%_)
                                   (cons (cons (gx#datum->syntax '#f 'def)
                                               (cons (cons _%g11531175%_
                                                           (cons (gx#datum->syntax
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                          '#f
                          'value)
                         '()))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                     (cons (cons _%L1096%_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (cons (gx#datum->syntax '#f 'value)
                               (cons (cons (gx#datum->syntax '#f 'quote)
                                           (cons _%g11521173%_ '()))
                                     (cons _%g11511170%_ '()))))
                   '())))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                         _%g11541177%_))
                                 '()
                                 _%L1090%_
                                 _%L1092%_
                                 _%L1093%_))
                        _%L1094%_
                        _%L1095%_)))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                         _%default-value4541082%_
                                         _%optional-accessor-slot4551085%_
                                         _%optional-accessor-name4561087%_
                                         _%required-accessor-slot430996%_
                                         _%required-accessor-name431999%_
                                         _%hd413922%_
                                         _%prototype-slot405894%_
                                         _%hd395862%_)
                                        (_%g265821%_ _%g267825%_))
                                    (_%g265821%_ _%g267825%_)))))))
                (_%loop4471028%_ _%target4441022%_ '() '() '()))
              (_%g265821%_ _%g267825%_)))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                            (_%g265821%_ _%g267825%_))
                                        (_%g265821%_ _%g267825%_))
                                    (_%g265821%_ _%g267825%_))))
                            (_%g265821%_ _%g267825%_))))
                    (_%g265821%_ _%g267825%_)))))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                        (_%loop424954%_
                                         _%target421948%_
                                         '()
                                         '()))
                                      (_%g265821%_ _%g267825%_)))))
                            (_%g265821%_ _%g267825%_))
                        (_%g265821%_ _%g267825%_))
                    (_%g265821%_ _%g267825%_))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g265821%_
                                                     _%g267825%_))))
                                            (_%g265821%_ _%g267825%_))))
                                    (_%g265821%_ _%g267825%_))
                                (_%g265821%_ _%g267825%_))
                            (_%g265821%_ _%g267825%_))))
                    (_%g265821%_ _%g267825%_))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g265821%_
                                                     _%g267825%_)))))))
                                (_%loop400874%_ _%target397868%_ '()))
                              (_%g265821%_ _%g267825%_)))))
                    (_%g265821%_ _%g267825%_))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g265821%_ _%g267825%_))
                                                (_%g265821%_ _%g267825%_))
                                            (_%g265821%_ _%g267825%_))))
                                    (_%g265821%_ _%g267825%_))))
                            (_%g265821%_ _%g267825%_))))
                    (_%g265821%_ _%g267825%_))))
             (_%g2631621%_
              (lambda (_%g2671184%_)
                (if (gx#stx-pair? _%g2671184%_)
                    (let ((_%e2781187%_ (gx#syntax-e _%g2671184%_)))
                      (let ((_%hd2791191%_
                             (let ()
                               (declare (not safe))
                               (##car _%e2781187%_)))
                            (_%tl2801194%_
                             (let ()
                               (declare (not safe))
                               (##cdr _%e2781187%_))))
                        (if (gx#stx-pair? _%tl2801194%_)
                            (let ((_%e2811197%_ (gx#syntax-e _%tl2801194%_)))
                              (let ((_%hd2821201%_
                                     (let ()
                                       (declare (not safe))
                                       (##car _%e2811197%_)))
                                    (_%tl2831204%_
                                     (let ()
                                       (declare (not safe))
                                       (##cdr _%e2811197%_))))
                                (if (gx#stx-pair? _%hd2821201%_)
                                    (let ((_%e2841207%_
                                           (gx#syntax-e _%hd2821201%_)))
                                      (let ((_%hd2851211%_
                                             (let ()
                                               (declare (not safe))
                                               (##car _%e2841207%_)))
                                            (_%tl2861214%_
                                             (let ()
                                               (declare (not safe))
                                               (##cdr _%e2841207%_))))
                                        (if (gx#identifier? _%hd2851211%_)
                                            (if (gx#free-identifier=?
                                                 |asp-gerbil-scheme/src/object-family/syntax[1]#_g48213_|
                                                 _%hd2851211%_)
                                                (if (gx#stx-pair?
                                                     _%tl2861214%_)
                                                    (let ((_%e2871217%_
                                                           (gx#syntax-e
                                                            _%tl2861214%_)))
                                                      (let ((_%hd2881221%_
                                                             (let ()
                                                               (declare
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (not safe))
                       (##car _%e2871217%_)))
                    (_%tl2891224%_
                     (let () (declare (not safe)) (##cdr _%e2871217%_))))
                (if (gx#stx-pair/null? _%tl2891224%_)
                    (let ((_g48214_ (gx#syntax-split-splice _%tl2891224%_ '0)))
                      (begin
                        (let ((_g48215_
                               (let ()
                                 (declare (not safe))
                                 (if (##values? _g48214_)
                                     (##values-length _g48214_)
                                     1))))
                          (if (not (let ()
                                     (declare (not safe))
                                     (##fx= _g48215_ 2)))
                              (error "Context expects 2 values" _g48215_)))
                        (let ((_%target2901227%_
                               (let ()
                                 (declare (not safe))
                                 (##values-ref _g48214_ 0)))
                              (_%tl2921230%_
                               (let ()
                                 (declare (not safe))
                                 (##values-ref _g48214_ 1))))
                          (if (gx#stx-null? _%tl2921230%_)
                              (letrec ((_%loop2931233%_
                                        (lambda (_%hd2911237%_
                                                 _%prototype-slot2971240%_)
                                          (if (gx#stx-pair? _%hd2911237%_)
                                              (let ((_%e2941243%_
                                                     (gx#syntax-e
                                                      _%hd2911237%_)))
                                                (let ((_%lp-hd2951247%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##car _%e2941243%_)))
                                                      (_%lp-tl2961250%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##cdr _%e2941243%_))))
                                                  (_%loop2931233%_
                                                   _%lp-tl2961250%_
                                                   (cons _%lp-hd2951247%_
                                                         _%prototype-slot2971240%_))))
                                              (let ((_%prototype-slot2981253%_
                                                     (reverse _%prototype-slot2971240%_)))
                                                (if (gx#stx-pair?
                                                     _%tl2831204%_)
                                                    (let ((_%e2991257%_
                                                           (gx#syntax-e
                                                            _%tl2831204%_)))
                                                      (let ((_%hd3001261%_
                                                             (let ()
                                                               (declare
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (not safe))
                       (##car _%e2991257%_)))
                    (_%tl3011264%_
                     (let () (declare (not safe)) (##cdr _%e2991257%_))))
                (if (gx#stx-pair? _%hd3001261%_)
                    (let ((_%e3021267%_ (gx#syntax-e _%hd3001261%_)))
                      (let ((_%hd3031271%_
                             (let ()
                               (declare (not safe))
                               (##car _%e3021267%_)))
                            (_%tl3041274%_
                             (let ()
                               (declare (not safe))
                               (##cdr _%e3021267%_))))
                        (if (gx#identifier? _%hd3031271%_)
                            (if (gx#free-identifier=?
                                 |asp-gerbil-scheme/src/object-family/syntax[1]#_g48216_|
                                 _%hd3031271%_)
                                (if (gx#stx-pair? _%tl3041274%_)
                                    (let ((_%e3051277%_
                                           (gx#syntax-e _%tl3041274%_)))
                                      (let ((_%hd3061281%_
                                             (let ()
                                               (declare (not safe))
                                               (##car _%e3051277%_)))
                                            (_%tl3071284%_
                                             (let ()
                                               (declare (not safe))
                                               (##cdr _%e3051277%_))))
                                        (if (gx#stx-pair/null? _%tl3071284%_)
                                            (let ((_g48217_
                                                   (gx#syntax-split-splice
                                                    _%tl3071284%_
                                                    '0)))
                                              (begin
                                                (let ((_g48218_
                                                       (let ()
                                                         (declare (not safe))
                                                         (if (##values?
                                                              _g48217_)
                                                             (##values-length
                                                              _g48217_)
                                                             1))))
                                                  (if (not (let ()
                                                             (declare
                                                               (not safe))
                                                             (##fx= _g48218_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                            2)))
              (error "Context expects 2 values" _g48218_)))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                (let ((_%target3081287%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##values-ref
                                                          _g48217_
                                                          0)))
                                                      (_%tl3101290%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##values-ref
                                                          _g48217_
                                                          1))))
                                                  (if (gx#stx-null?
                                                       _%tl3101290%_)
                                                      (letrec ((_%loop3111293%_
                                                                (lambda (_%hd3091297%_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                                 _%constructor-slot3151300%_)
                          (if (gx#stx-pair? _%hd3091297%_)
                              (let ((_%e3121303%_ (gx#syntax-e _%hd3091297%_)))
                                (let ((_%lp-hd3131307%_
                                       (let ()
                                         (declare (not safe))
                                         (##car _%e3121303%_)))
                                      (_%lp-tl3141310%_
                                       (let ()
                                         (declare (not safe))
                                         (##cdr _%e3121303%_))))
                                  (_%loop3111293%_
                                   _%lp-tl3141310%_
                                   (cons _%lp-hd3131307%_
                                         _%constructor-slot3151300%_))))
                              (let ((_%constructor-slot3161313%_
                                     (reverse _%constructor-slot3151300%_)))
                                (if (gx#stx-pair? _%tl3011264%_)
                                    (let ((_%e3171317%_
                                           (gx#syntax-e _%tl3011264%_)))
                                      (let ((_%hd3181321%_
                                             (let ()
                                               (declare (not safe))
                                               (##car _%e3171317%_)))
                                            (_%tl3191324%_
                                             (let ()
                                               (declare (not safe))
                                               (##cdr _%e3171317%_))))
                                        (if (gx#stx-pair? _%hd3181321%_)
                                            (let ((_%e3201327%_
                                                   (gx#syntax-e
                                                    _%hd3181321%_)))
                                              (let ((_%hd3211331%_
                                                     (let ()
                                                       (declare (not safe))
                                                       (##car _%e3201327%_)))
                                                    (_%tl3221334%_
                                                     (let ()
                                                       (declare (not safe))
                                                       (##cdr _%e3201327%_))))
                                                (if (gx#identifier?
                                                     _%hd3211331%_)
                                                    (if (gx#free-identifier=?
                                                         |asp-gerbil-scheme/src/object-family/syntax[1]#_g48219_|
                                                         _%hd3211331%_)
                                                        (if (gx#stx-pair?
                                                             _%tl3221334%_)
                                                            (let ((_%e3231337%_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                           (gx#syntax-e _%tl3221334%_)))
                      (let ((_%hd3241341%_
                             (let ()
                               (declare (not safe))
                               (##car _%e3231337%_)))
                            (_%tl3251344%_
                             (let ()
                               (declare (not safe))
                               (##cdr _%e3231337%_))))
                        (if (gx#stx-pair? _%tl3251344%_)
                            (let ((_%e3261347%_ (gx#syntax-e _%tl3251344%_)))
                              (let ((_%hd3271351%_
                                     (let ()
                                       (declare (not safe))
                                       (##car _%e3261347%_)))
                                    (_%tl3281354%_
                                     (let ()
                                       (declare (not safe))
                                       (##cdr _%e3261347%_))))
                                (if (gx#stx-pair? _%hd3271351%_)
                                    (let ((_%e3291357%_
                                           (gx#syntax-e _%hd3271351%_)))
                                      (let ((_%hd3301361%_
                                             (let ()
                                               (declare (not safe))
                                               (##car _%e3291357%_)))
                                            (_%tl3311364%_
                                             (let ()
                                               (declare (not safe))
                                               (##cdr _%e3291357%_))))
                                        (if (gx#identifier? _%hd3301361%_)
                                            (if (gx#free-identifier=?
                                                 |asp-gerbil-scheme/src/object-family/syntax[1]#_g48220_|
                                                 _%hd3301361%_)
                                                (if (gx#stx-pair/null?
                                                     _%tl3311364%_)
                                                    (let ((_g48221_
                                                           (gx#syntax-split-splice
                                                            _%tl3311364%_
                                                            '0)))
                                                      (begin
                                                        (let ((_g48222_
                                                               (let ()
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (declare (not safe))
                         (if (##values? _g48221_)
                             (##values-length _g48221_)
                             1))))
                  (if (not (let () (declare (not safe)) (##fx= _g48222_ 2)))
                      (error "Context expects 2 values" _g48222_)))
                (let ((_%target3321367%_
                       (let () (declare (not safe)) (##values-ref _g48221_ 0)))
                      (_%tl3341370%_
                       (let ()
                         (declare (not safe))
                         (##values-ref _g48221_ 1))))
                  (if (gx#stx-null? _%tl3341370%_)
                      (letrec ((_%loop3351373%_
                                (lambda (_%hd3331377%_
                                         _%required-accessor-slot3391380%_
                                         _%required-accessor-name3401382%_)
                                  (if (gx#stx-pair? _%hd3331377%_)
                                      (let ((_%e3361385%_
                                             (gx#syntax-e _%hd3331377%_)))
                                        (let ((_%lp-hd3371389%_
                                               (let ()
                                                 (declare (not safe))
                                                 (##car _%e3361385%_)))
                                              (_%lp-tl3381392%_
                                               (let ()
                                                 (declare (not safe))
                                                 (##cdr _%e3361385%_))))
                                          (if (gx#stx-pair? _%lp-hd3371389%_)
                                              (let ((_%e3431395%_
                                                     (gx#syntax-e
                                                      _%lp-hd3371389%_)))
                                                (let ((_%hd3441399%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##car _%e3431395%_)))
                                                      (_%tl3451402%_
                                                       (let ()
                                                         (declare (not safe))
                                                         (##cdr _%e3431395%_))))
                                                  (if (gx#stx-pair?
                                                       _%tl3451402%_)
                                                      (let ((_%e3461405%_
                                                             (gx#syntax-e
                                                              _%tl3451402%_)))
                                                        (let ((_%hd3471409%_
                                                               (let ()
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (declare (not safe))
                         (##car _%e3461405%_)))
                      (_%tl3481412%_
                       (let () (declare (not safe)) (##cdr _%e3461405%_))))
                  (if (gx#stx-null? _%tl3481412%_)
                      (_%loop3351373%_
                       _%lp-tl3381392%_
                       (cons _%hd3471409%_ _%required-accessor-slot3391380%_)
                       (cons _%hd3441399%_ _%required-accessor-name3401382%_))
                      (_%g2641180%_ _%g2671184%_))))
              (_%g2641180%_ _%g2671184%_))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                              (_%g2641180%_ _%g2671184%_))))
                                      (let ((_%required-accessor-slot3411415%_
                                             (reverse _%required-accessor-slot3391380%_))
                                            (_%required-accessor-name3421418%_
                                             (reverse _%required-accessor-name3401382%_)))
                                        (if (gx#stx-pair? _%tl3281354%_)
                                            (let ((_%e3491421%_
                                                   (gx#syntax-e
                                                    _%tl3281354%_)))
                                              (let ((_%hd3501425%_
                                                     (let ()
                                                       (declare (not safe))
                                                       (##car _%e3491421%_)))
                                                    (_%tl3511428%_
                                                     (let ()
                                                       (declare (not safe))
                                                       (##cdr _%e3491421%_))))
                                                (if (gx#stx-pair?
                                                     _%hd3501425%_)
                                                    (let ((_%e3521431%_
                                                           (gx#syntax-e
                                                            _%hd3501425%_)))
                                                      (let ((_%hd3531435%_
                                                             (let ()
                                                               (declare
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (not safe))
                       (##car _%e3521431%_)))
                    (_%tl3541438%_
                     (let () (declare (not safe)) (##cdr _%e3521431%_))))
                (if (gx#identifier? _%hd3531435%_)
                    (if (gx#free-identifier=?
                         |asp-gerbil-scheme/src/object-family/syntax[1]#_g48223_|
                         _%hd3531435%_)
                        (if (gx#stx-pair/null? _%tl3541438%_)
                            (let ((_g48224_
                                   (gx#syntax-split-splice _%tl3541438%_ '0)))
                              (begin
                                (let ((_g48225_
                                       (let ()
                                         (declare (not safe))
                                         (if (##values? _g48224_)
                                             (##values-length _g48224_)
                                             1))))
                                  (if (not (let ()
                                             (declare (not safe))
                                             (##fx= _g48225_ 2)))
                                      (error "Context expects 2 values"
                                             _g48225_)))
                                (let ((_%target3551441%_
                                       (let ()
                                         (declare (not safe))
                                         (##values-ref _g48224_ 0)))
                                      (_%tl3571444%_
                                       (let ()
                                         (declare (not safe))
                                         (##values-ref _g48224_ 1))))
                                  (if (gx#stx-null? _%tl3571444%_)
                                      (letrec ((_%loop3581447%_
                                                (lambda (_%hd3561451%_
                                                         _%default-value3621454%_
                                                         _%optional-accessor-slot3631456%_
                                                         _%optional-accessor-name3641458%_)
                                                  (if (gx#stx-pair?
                                                       _%hd3561451%_)
                                                      (let ((_%e3591461%_
                                                             (gx#syntax-e
                                                              _%hd3561451%_)))
                                                        (let ((_%lp-hd3601465%_
                                                               (let ()
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         (declare (not safe))
                         (##car _%e3591461%_)))
                      (_%lp-tl3611468%_
                       (let () (declare (not safe)) (##cdr _%e3591461%_))))
                  (if (gx#stx-pair? _%lp-hd3601465%_)
                      (let ((_%e3681471%_ (gx#syntax-e _%lp-hd3601465%_)))
                        (let ((_%hd3691475%_
                               (let ()
                                 (declare (not safe))
                                 (##car _%e3681471%_)))
                              (_%tl3701478%_
                               (let ()
                                 (declare (not safe))
                                 (##cdr _%e3681471%_))))
                          (if (gx#stx-pair? _%tl3701478%_)
                              (let ((_%e3711481%_ (gx#syntax-e _%tl3701478%_)))
                                (let ((_%hd3721485%_
                                       (let ()
                                         (declare (not safe))
                                         (##car _%e3711481%_)))
                                      (_%tl3731488%_
                                       (let ()
                                         (declare (not safe))
                                         (##cdr _%e3711481%_))))
                                  (if (gx#stx-pair? _%tl3731488%_)
                                      (let ((_%e3741491%_
                                             (gx#syntax-e _%tl3731488%_)))
                                        (let ((_%hd3751495%_
                                               (let ()
                                                 (declare (not safe))
                                                 (##car _%e3741491%_)))
                                              (_%tl3761498%_
                                               (let ()
                                                 (declare (not safe))
                                                 (##cdr _%e3741491%_))))
                                          (if (gx#stx-null? _%tl3761498%_)
                                              (_%loop3581447%_
                                               _%lp-tl3611468%_
                                               (cons _%hd3751495%_
                                                     _%default-value3621454%_)
                                               (cons _%hd3721485%_
                                                     _%optional-accessor-slot3631456%_)
                                               (cons _%hd3691475%_
                                                     _%optional-accessor-name3641458%_))
                                              (_%g2641180%_ _%g2671184%_))))
                                      (_%g2641180%_ _%g2671184%_))))
                              (_%g2641180%_ _%g2671184%_))))
                      (_%g2641180%_ _%g2671184%_))))
              (let ((_%default-value3651501%_
                     (reverse _%default-value3621454%_))
                    (_%optional-accessor-slot3661504%_
                     (reverse _%optional-accessor-slot3631456%_))
                    (_%optional-accessor-name3671506%_
                     (reverse _%optional-accessor-name3641458%_)))
                (if (gx#stx-null? _%tl3511428%_)
                    (if (gx#stx-null? _%tl3191324%_)
                        ((lambda (_%L1509%_
                                  _%L1511%_
                                  _%L1512%_
                                  _%L1513%_
                                  _%L1514%_
                                  _%L1515%_
                                  _%L1516%_
                                  _%L1517%_
                                  _%L1518%_
                                  _%L1519%_)
                           (cons (gx#datum->syntax '#f 'begin)
                                 (cons (cons (gx#datum->syntax '#f '.def)
                                             (cons _%L1519%_
                                                   (foldr (lambda (_%g15791591%_
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                           _%g15801594%_)
                    (cons _%g15791591%_ _%g15801594%_))
                  '()
                  _%L1518%_)))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                       (cons (cons (gx#datum->syntax '#f 'def)
                                                   (cons _%L1517%_
                                                         (cons (cons (gx#datum->syntax
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                              '#f
                              '.o)
                             (cons (cons '::
                                         (cons (gx#datum->syntax '#f '@)
                                               (cons _%L1519%_ '())))
                                   (foldr (lambda (_%g15811597%_ _%g15821600%_)
                                            (cons _%g15811597%_ _%g15821600%_))
                                          '()
                                          _%L1516%_)))
                       '())))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                             (begin
                                               (gx#syntax-check-splice-targets
                                                _%L1513%_
                                                _%L1514%_)
                                               (foldr (lambda (_%g15831603%_
                                                               _%g15841606%_
                                                               _%g15851608%_)
                                                        (cons (cons (gx#datum->syntax
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                             '#f
                             'def)
                            (cons (cons _%g15841606%_
                                        (cons (gx#datum->syntax '#f 'value)
                                              '()))
                                  (cons (cons _%L1515%_
                                              (cons (gx#datum->syntax
                                                     '#f
                                                     'value)
                                                    (cons (cons (gx#datum->syntax
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                         '#f
                         'quote)
                        (cons _%g15831603%_ '()))
                  '())))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                        '())))
                      _%g15851608%_))
              (begin
                (gx#syntax-check-splice-targets _%L1509%_ _%L1511%_ _%L1512%_)
                (foldr (lambda (_%g15861611%_
                                _%g15871614%_
                                _%g15881616%_
                                _%g15891618%_)
                         (cons (cons (gx#datum->syntax '#f 'def)
                                     (cons (cons _%g15881616%_
                                                 (cons (gx#datum->syntax
                                                        '#f
                                                        'value)
                                                       '()))
                                           (cons (cons _%L1515%_
                                                       (cons (gx#datum->syntax
                                                              '#f
                                                              'value)
                                                             (cons (cons (gx#datum->syntax
;;<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
                                  '#f
                                  'quote)
                                 (cons _%g15871614%_ '()))
                           (cons _%g15861611%_ '()))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                 '())))
                               _%g15891618%_))
                       '()
                       _%L1509%_
                       _%L1511%_
                       _%L1512%_))
              _%L1513%_
              _%L1514%_))))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                         _%default-value3651501%_
                         _%optional-accessor-slot3661504%_
                         _%optional-accessor-name3671506%_
                         _%required-accessor-slot3411415%_
                         _%required-accessor-name3421418%_
                         _%hd3241341%_
                         _%constructor-slot3161313%_
                         _%hd3061281%_
                         _%prototype-slot2981253%_
                         _%hd2881221%_)
                        (_%g2641180%_ _%g2671184%_))
                    (_%g2641180%_ _%g2671184%_)))))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                        (_%loop3581447%_
                                         _%target3551441%_
                                         '()
                                         '()
                                         '()))
                                      (_%g2641180%_ _%g2671184%_)))))
                            (_%g2641180%_ _%g2671184%_))
                        (_%g2641180%_ _%g2671184%_))
                    (_%g2641180%_ _%g2671184%_))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g2641180%_
                                                     _%g2671184%_))))
                                            (_%g2641180%_ _%g2671184%_)))))))
                        (_%loop3351373%_ _%target3321367%_ '() '()))
                      (_%g2641180%_ _%g2671184%_)))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g2641180%_
                                                     _%g2671184%_))
                                                (_%g2641180%_ _%g2671184%_))
                                            (_%g2641180%_ _%g2671184%_))))
                                    (_%g2641180%_ _%g2671184%_))))
                            (_%g2641180%_ _%g2671184%_))))
                    (_%g2641180%_ _%g2671184%_))
                (_%g2641180%_ _%g2671184%_))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g2641180%_
                                                     _%g2671184%_))))
                                            (_%g2641180%_ _%g2671184%_))))
                                    (_%g2641180%_ _%g2671184%_)))))))
                (_%loop3111293%_ _%target3081287%_ '()))
              (_%g2641180%_ _%g2671184%_)))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                            (_%g2641180%_ _%g2671184%_))))
                                    (_%g2641180%_ _%g2671184%_))
                                (_%g2641180%_ _%g2671184%_))
                            (_%g2641180%_ _%g2671184%_))))
                    (_%g2641180%_ _%g2671184%_))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g2641180%_
                                                     _%g2671184%_)))))))
                                (_%loop2931233%_ _%target2901227%_ '()))
                              (_%g2641180%_ _%g2671184%_)))))
                    (_%g2641180%_ _%g2671184%_))))
;;>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
                                                    (_%g2641180%_
                                                     _%g2671184%_))
                                                (_%g2641180%_ _%g2671184%_))
                                            (_%g2641180%_ _%g2671184%_))))
                                    (_%g2641180%_ _%g2671184%_))))
                            (_%g2641180%_ _%g2671184%_))))
                    (_%g2641180%_ _%g2671184%_)))))
        (_%g2631621%_ _%$stx260%_)))))
