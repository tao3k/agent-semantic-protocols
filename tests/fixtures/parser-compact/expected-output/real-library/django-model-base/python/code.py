class ModelBase
  def __new__
    assign super_new
    assign parents
    if not parents
      return super_new
    assign module
    assign new_attrs
    assign classcell
    if classcell is not None
      assign new_attrs['__classcell__']
    assign attr_meta
    assign contributable_attrs
    for (obj_name, obj) in items
      if _has_contribute_to_class
        assign contributable_attrs[obj_name]
        assign new_attrs[obj_name]
    assign new_class
    assign abstract
    assign meta
    assign base_meta
    assign app_label
    assign app_config
    if getattr(meta, 'app_label', None) is None
