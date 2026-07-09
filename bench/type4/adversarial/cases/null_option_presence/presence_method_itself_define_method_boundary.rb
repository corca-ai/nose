m = method(:define_method).itself

m.call(:nil?) do
  true
end

def rb_method_itself_define_method_redefined(value, other)
  value.nil?
end
