(declare (block) (standard-bindings) (extended-bindings))
(begin
  (define asp-gerbil-scheme/src/object-family/syntax::timestamp 1789096509)
  (begin
    (define asp-gerbil-scheme/src/object-family/syntax#absent-poo-family-slot
      (cons 'absent-poo-family-slot '()))
    (define asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
      (let ((_%opt-lambda16331639%_
             (lambda (_%value1635%_ _%key1636%_ _%default1637%_)
               (if (clan/poo/object#.slot? _%value1635%_ _%key1636%_)
                   (clan/poo/object#.ref _%value1635%_ _%key1636%_)
                   (if (eq? _%default1637%_
                            asp-gerbil-scheme/src/object-family/syntax#absent-poo-family-slot)
                       (clan/poo/object#.ref _%value1635%_ _%key1636%_)
                       _%default1637%_)))))
        (lambda _g48194_
          (let ((_g48195_ (let () (declare (not safe)) (##length _g48194_))))
            (cond ((let () (declare (not safe)) (##fx= _g48195_ 2))
                   (apply (lambda (_%value1642%_ _%key1643%_)
                            (let ((_%default1645%_
                                   asp-gerbil-scheme/src/object-family/syntax#absent-poo-family-slot))
                              (_%opt-lambda16331639%_
                               _%value1642%_
                               _%key1643%_
                               _%default1645%_)))
                          _g48194_))
                  ((let () (declare (not safe)) (##fx= _g48195_ 3))
                   (apply _%opt-lambda16331639%_ _g48194_))
                  (else
                   (##raise-wrong-number-of-arguments-exception
                    asp-gerbil-scheme/src/object-family/syntax#poo-family-ref
                    _g48194_)))))))))
