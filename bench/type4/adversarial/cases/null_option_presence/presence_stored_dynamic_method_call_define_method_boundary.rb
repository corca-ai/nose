mutator = :define_method
m = method(mutator)

m.call(:nil?) do
  true
end

def rb_stored_dynamic_method_call_define_method_redefined(value, other)
  value.nil?
end
