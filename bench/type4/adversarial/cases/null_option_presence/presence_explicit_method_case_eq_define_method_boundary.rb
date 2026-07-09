m = method(:define_method)

m.===(:nil?) do
  true
end

def rb_explicit_method_case_eq_define_method_redefined(value, other)
  value.nil?
end
