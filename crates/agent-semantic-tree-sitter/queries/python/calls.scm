;; Python call target captures projected from ast.Call.

(call
  function: (_) @call.target) @call.expression

(call
  function: (attribute
    attribute: (identifier) @call.method)) @call.expression

(keyword_argument
  name: (identifier) @call.keyword)
