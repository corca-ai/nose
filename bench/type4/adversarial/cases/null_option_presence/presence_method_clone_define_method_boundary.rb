m = method(:define_method).clone

m.call(:nil?) do
  true
end

def rb_method_clone_define_method_redefined(value, other)
  value.nil?
end
