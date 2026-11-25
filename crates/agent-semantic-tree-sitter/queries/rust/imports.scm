(use_declaration
  argument: (identifier) @import.path) @import.declaration

(use_declaration
  argument: (scoped_identifier) @import.path) @import.declaration

(use_declaration
  argument: (use_list) @import.path) @import.declaration

(use_declaration
  argument: (scoped_use_list) @import.path) @import.declaration

(use_declaration
  (visibility_modifier) @import.visibility)

(use_declaration
  argument: (use_as_clause
    path: (_) @import.path
    alias: (identifier) @import.alias)) @import.declaration

(extern_crate_declaration
  name: (identifier) @import.path) @import.declaration

(extern_crate_declaration
  alias: (identifier) @import.alias)
