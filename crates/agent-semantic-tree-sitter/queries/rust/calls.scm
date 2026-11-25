(call_expression
  function: (identifier) @call.target) @call.expression

(call_expression
  function: (scoped_identifier) @call.target) @call.expression

(call_expression
  function: (field_expression
    field: (field_identifier) @call.method) @call.target) @call.expression

(call_expression
  function: (generic_function
    function: (identifier) @call.target)) @call.expression

(call_expression
  function: (generic_function
    function: (scoped_identifier) @call.target)) @call.expression

(call_expression
  function: (generic_function
    function: (field_expression
      field: (field_identifier) @call.method) @call.target)) @call.expression
