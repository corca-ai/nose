mutator = :define_method

send(mutator, :nil?) do
  true
end

def rb_dynamic_reflective_define_method_redefined(value, other)
  value.nil?
end
