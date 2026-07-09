m = Module.send(:instance_method, :define_method).bind(Object)

m.call(:nil?) do
  true
end

def rb_send_acquired_unbound_method_define_method_redefined(value, other)
  value.nil?
end
