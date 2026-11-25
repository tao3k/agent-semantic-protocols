;; Python import binding captures projected from ast.Import and ast.ImportFrom.

(import_statement
  name: (dotted_name) @import.name) @import.declaration

(import_from_statement
  module_name: (dotted_name)? @import.path
  name: (dotted_name) @import.name) @import.declaration

(aliased_import
  name: (dotted_name) @import.name
  alias: (identifier) @import.alias)
