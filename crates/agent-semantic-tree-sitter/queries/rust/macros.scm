(macro_invocation
  macro: (identifier) @macro.name) @macro.invocation

(macro_invocation
  macro: (scoped_identifier) @macro.name) @macro.invocation

(macro_invocation
  (token_tree) @macro.arguments)
