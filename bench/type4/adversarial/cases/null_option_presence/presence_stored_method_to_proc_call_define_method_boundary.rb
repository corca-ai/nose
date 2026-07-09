p = method(:define_method).to_proc

p.call(:nil?) do
  true
end

def rb_stored_method_to_proc_call_define_method_redefined(value, other)
  value.nil?
end
