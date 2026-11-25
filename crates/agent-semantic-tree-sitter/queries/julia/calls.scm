; ASP semantic calls for JuliaSyntax native projection.

(call_expression) @call.expression

(call_expression
  (identifier) @call.target)

(call_expression
  (field_expression
    (identifier)
    (identifier) @call.method))
