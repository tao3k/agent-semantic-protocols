; ASP semantic declarations for JuliaSyntax native projection.

(function_definition) @function.definition

(function_definition
  (signature
    (identifier) @function.name))

(function_definition
  (signature
    (call_expression
      (identifier) @function.name)))

(macro_definition) @macro.definition

(macro_definition
  (signature
    (call_expression
      (identifier) @macro.name)))

(module_definition) @module.definition

(module_definition
  name: (identifier) @module.name)

[
  (abstract_definition)
  (primitive_definition)
  (struct_definition)
] @type.definition

(type_head
  (identifier) @type.name)

(assignment
  .
  (identifier) @constant.name) @constant.definition
