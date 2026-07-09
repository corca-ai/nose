m = method(:define_method)
x = [m].first

x.call(:nil?) do
  true
end

def rb_array_transferred_method_define_method_redefined(value, other)
  value.nil?
end
