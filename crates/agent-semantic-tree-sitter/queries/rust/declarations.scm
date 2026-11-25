(function_item
  name: (identifier) @function.name) @function.definition

(function_item
  (visibility_modifier) @item.visibility)

(function_item
  (function_modifiers) @function.modifier)

(function_item
  type_parameters: (type_parameters) @function.type_parameters)

(function_item
  return_type: (_) @function.return_type)

(function_item
  return_type: (generic_type) @function.return_type)

(struct_item
  name: (type_identifier) @type.name) @type.definition

(struct_item
  (visibility_modifier) @item.visibility)

(struct_item
  type_parameters: (type_parameters) @type.type_parameters)

(enum_item
  name: (type_identifier) @type.name) @type.definition

(enum_item
  (visibility_modifier) @item.visibility)

(enum_item
  type_parameters: (type_parameters) @type.type_parameters)

(union_item
  name: (type_identifier) @type.name) @type.definition

(union_item
  (visibility_modifier) @item.visibility)

(union_item
  type_parameters: (type_parameters) @type.type_parameters)

(trait_item
  name: (type_identifier) @trait.name) @trait.definition

(trait_item
  (visibility_modifier) @item.visibility)

(trait_item
  type_parameters: (type_parameters) @trait.type_parameters)

(trait_item
  bounds: (trait_bounds) @trait.bounds)

(impl_item
  type: (_) @impl.target) @impl.definition

(impl_item
  type: (generic_type) @impl.target) @impl.definition

(impl_item
  trait: (_) @impl.trait)

(impl_item
  trait: (generic_type) @impl.trait)

(impl_item
  type_parameters: (type_parameters) @impl.type_parameters)

(type_item
  name: (type_identifier) @type.name) @type.definition

(type_item
  (visibility_modifier) @item.visibility)

(type_item
  type_parameters: (type_parameters) @type.type_parameters)

(type_item
  type: (_) @type.aliased_type)

(type_item
  type: (generic_type) @type.aliased_type)

(const_item
  name: (identifier) @constant.name) @constant.definition

(const_item
  (visibility_modifier) @item.visibility)

(const_item
  type: (_) @constant.type)

(const_item
  type: (primitive_type) @constant.type)

(static_item
  name: (identifier) @constant.name) @constant.definition

(static_item
  (visibility_modifier) @item.visibility)

(static_item
  type: (_) @constant.type)

(static_item
  type: (primitive_type) @constant.type)

(mod_item
  name: (identifier) @module.name) @module.definition

(mod_item
  (visibility_modifier) @item.visibility)

(attribute_item) @item.attribute
