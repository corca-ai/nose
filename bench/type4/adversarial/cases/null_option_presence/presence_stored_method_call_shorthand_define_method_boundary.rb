m = method(:define_method)

m.(:nil?) do
  true
end

def rb_stored_method_call_shorthand_define_method_redefined(value, other)
  value.nil?
end
