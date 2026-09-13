;;; ASP's default Search Playbook declaration.
;;;
;;; The macro and all POO Flow lowering live in MRR.  This file contains
;;; only the readable search policy selected by ASP.

(import :meta-relational-reasoning/scheme/search/playbook)
(export asp-search-playbook)

(defsearch-playbook asp-search-playbook
  (producers
   (language rust python typescript)
   (documents org md))
  (chain
   (intersect
    (rg "-n"
        "-g" "*.rs" "-g" "*.py" "-g" "*.org"
        "-e" "refresh|publish|artifactDigest|rank"
        "crates" "languages" "docs" "schemas")
    (tantivy
     "(title:\"artifact refresh\"^2 OR body:\"artifact publication\"^2) AND (runtime OR registry)"))
   (syntax rust "(function_item name: (identifier) @name)")))
