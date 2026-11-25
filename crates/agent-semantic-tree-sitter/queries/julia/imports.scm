; ASP semantic imports for JuliaSyntax native projection.

(import_statement) @import.declaration

(import_statement
  (identifier) @import.module)

(import_statement
  (selected_import
    (identifier) @import.name))

(using_statement) @using.declaration

(using_statement
  (identifier) @using.module)

(using_statement
  (selected_import
    (identifier) @using.name))
