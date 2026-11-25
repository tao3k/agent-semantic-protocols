;; Python decorator captures projected from decorated ast definitions.

(decorated_definition
  (decorator) @decorator.expression
  definition: (_) @decorator.target)

(decorator
  (call
    function: (_) @decorator.call))
