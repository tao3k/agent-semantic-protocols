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
      if app_config is None
        if not abstract
          raise RuntimeError
        assign app_label
    call add_to_class:_meta
    if not abstract
      call add_to_class:DoesNotExist
      call add_to_class:MultipleObjectsReturned
      call add_to_class:NotUpdated
      if base_meta and (not base_meta.abstract)
        if not hasattr(meta, 'ordering')
          assign new_class._meta.ordering
        if not hasattr(meta, 'get_latest_by')
          assign new_class._meta.get_latest_by
    assign is_proxy
    if is_proxy and base_meta and base_meta.swapped
      raise TypeError
    for (obj_name, obj) in items
      call add_to_class
    assign new_fields
    assign field_names
    if is_proxy
      assign base
      for parent in [kls for kls in parents if hasattr(kls, '_meta')]
        if parent._meta.abstract
          if parent._meta.fields
            raise TypeError
            continue
        if base is None
          assign base
          if parent._meta.concrete_model is not base._meta.concrete_model
            raise TypeError
      if base is None
        raise TypeError
      call setup_proxy
      assign new_class._meta.concrete_model
      assign new_class._meta.concrete_model
    assign parent_links
    for base in reversed
      if not hasattr(base, '_meta')
        continue
      if base != new_class and (not base._meta.abstract)
        continue
      for field in base._meta.local_fields
        if isinstance(field, OneToOneField) and field.remote_field.parent_link
          assign related
          assign parent_links[make_model_tuple(related)]
    assign inherited_attributes
    for base in mro
      if base not in parents or not hasattr(base, '_meta')
        call update
        continue
      assign parent_fields
      if not base._meta.abstract
        for field in parent_fields
          if field.name in field_names
            raise FieldError
            call add
        assign base
        assign base_key
        if base_key in parent_links
          assign field
          if not is_proxy
            assign attr_name
            assign field
            if attr_name in field_names
              raise FieldError
            if not hasattr(new_class, attr_name)
              call add_to_class
            assign field
        assign new_class._meta.parents[base]
        assign base_parents
        for field in parent_fields
          if field.name not in field_names and field.name not in new_class.__dict__ and (field.name not in inherited_attributes)
            assign new_field
            call add_to_class
            if field.one_to_one
              for (parent, parent_link) in items
                if field == parent_link
                  assign base_parents[parent]
        call update
      for field in base._meta.private_fields
        if field.name in field_names
          if not base._meta.abstract
            raise FieldError
          if field.name not in new_class.__dict__ and field.name not in inherited_attributes
            assign field
            if not base._meta.abstract
              assign field.mti_inherited
            call add_to_class
    assign new_class._meta.indexes
    if abstract
      assign attr_meta.abstract
      assign new_class.Meta
      return new_class
    call _prepare
    call register_model
    return new_class
  def add_to_class
    if _has_contribute_to_class
      call contribute_to_class
      call setattr
  def _prepare
    assign opts
    call _prepare
    if opts.order_with_respect_to
      assign cls.get_next_in_order
      assign cls.get_previous_in_order
      if opts.order_with_respect_to.remote_field
        assign wrt
        assign remote
        call lazy_related_operation
    if cls.__doc__ is None
      assign cls.__doc__
    assign get_absolute_url_override
    if get_absolute_url_override
      call setattr
    if not opts.managers
      if any
        raise ValueError
      assign manager
      assign manager.auto_created
      call add_to_class:objects
    for index in cls._meta.indexes
      if not index.name
        call set_name_with_model
    call send
  def _base_manager
    @property
    return cls._meta.base_manager
  def _default_manager
    @property
    return cls._meta.default_manager
