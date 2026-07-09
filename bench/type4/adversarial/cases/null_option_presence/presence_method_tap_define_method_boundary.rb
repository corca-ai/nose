m = method(:define_method).tap {}

m.call(:nil?) do
  true
end

def rb_method_tap_define_method_redefined(value, other)
  value.nil?
end
