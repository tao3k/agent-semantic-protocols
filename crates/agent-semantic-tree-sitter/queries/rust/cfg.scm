(attribute_item
  (attribute
    (identifier) @attribute.name) @attribute.body) @attribute.item

(attribute_item
  (attribute
    arguments: (token_tree) @attribute.arguments))

(attribute_item
  (attribute
    value: (_) @attribute.value))
