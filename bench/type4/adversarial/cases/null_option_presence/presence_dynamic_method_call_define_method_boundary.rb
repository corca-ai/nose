mutator = :define_method

method(mutator).call(:nil?) do
  true
end

def rb_dynamic_method_call_define_method_redefined(value, other)
  value.nil?
end
