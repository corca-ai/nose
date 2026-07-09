m = send(:method, :define_method)

m.call(:nil?) do
  true
end

def rb_send_acquired_method_define_method_redefined(value, other)
  value.nil?
end
