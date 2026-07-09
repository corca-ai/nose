m = method(:define_method).unbind.bind(self)

m.call(:nil?) do
  true
end

def rb_unbind_bind_define_method_redefined(value, other)
  value.nil?
end
