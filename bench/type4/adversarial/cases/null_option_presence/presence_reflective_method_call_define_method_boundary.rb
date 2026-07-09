m = method(:define_method)

m.send(:call, :nil?) do
  true
end

def rb_reflective_method_call_define_method_redefined(value, other)
  value.nil?
end
