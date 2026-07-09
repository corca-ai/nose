method(:define_method).call(:nil?) do
  true
end

def rb_method_call_define_method_redefined(value, other)
  value.nil?
end
