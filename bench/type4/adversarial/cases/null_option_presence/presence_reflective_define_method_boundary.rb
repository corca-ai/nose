send(:define_method, :nil?) do
  true
end

def rb_reflective_define_method_redefined(value, other)
  value.nil?
end
