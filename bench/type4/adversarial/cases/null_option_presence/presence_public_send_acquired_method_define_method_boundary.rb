m = public_send(:method, :define_method)

m.call(:nil?) do
  true
end

def rb_public_send_acquired_method_define_method_redefined(value, other)
  value.nil?
end
