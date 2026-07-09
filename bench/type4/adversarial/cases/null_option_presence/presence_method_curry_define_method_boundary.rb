p = method(:define_method).curry

p.call(:nil?) do
  true
end

def rb_method_curry_define_method_redefined(value, other)
  value.nil?
end
