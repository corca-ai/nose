class Object
  method_name = :nil?
  alias_method method_name, :object_id
end

def rb_dynamic_alias_method_redefined(value, other)
  value.nil?
end
