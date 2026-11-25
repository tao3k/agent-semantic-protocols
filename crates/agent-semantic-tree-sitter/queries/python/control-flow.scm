;; Python control-flow captures projected from native ast statements.

(if_statement
  condition: (_) @control.condition) @control.if

(for_statement
  left: (_) @control.target
  right: (_) @control.iterable) @control.loop

(while_statement
  condition: (_) @control.condition) @control.loop

(with_statement
  (with_item) @context.manager) @control.with

(try_statement) @control.exception

(match_statement
  subject: (_) @control.subject) @control.match
