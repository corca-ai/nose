m = method(:define_method)
x = {k: m}[:k]

x.call(:nil?) do
  true
end

def rb_hash_transferred_method_define_method_redefined(value, other)
  value.nil?
end
