fn deserialize_body
if cont.attrs.transparent()
return deserialize_transparent
else
if let Some(type_from) = cont.attrs.type_from()
return deserialize_from
else
if let Some(type_try_from) = cont.attrs.type_try_from()
return deserialize_try_from
else
if let attr::Identifier::No = cont.attrs.identifier()
match &cont.data
case Data::Enum(variants)
call enum_::deserialize
case Data::Struct(Style::Struct, fields)
return struct_::deserialize
case Data::Struct(Style::Tuple, fields) | Data::Struct(Style::Newtype, fields)
return tuple::deserialize
case Data::Struct(Style::Unit, _)
call unit::deserialize
else
match &cont.data
case Data::Enum(variants)
call identifier::deserialize_custom
case Data::Struct(_, _)
call unreachable!("checked in serde_derive_internals")
