method_name = :nil?

define_method(method_name) do
  true
end

def rb_dynamic_define_method_redefined(value, other)
  value.nil?
end
