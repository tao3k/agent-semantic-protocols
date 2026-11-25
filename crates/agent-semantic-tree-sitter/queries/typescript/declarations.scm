(function_declaration name: (identifier) @function.name) @function.definition
(class_declaration name: (type_identifier) @class.name) @class.definition
(interface_declaration name: (type_identifier) @interface.name) @interface.definition
(type_alias_declaration name: (type_identifier) @type.name) @type.definition
(enum_declaration name: (identifier) @enum.name) @enum.definition
(lexical_declaration (variable_declarator name: (identifier) @variable.name)) @variable.definition
(import_statement source: (string) @import.source) @import.declaration
(export_statement) @export.declaration
