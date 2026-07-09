m = method(:define_method)
p = m.to_proc

p.call(:nil?) do
  true
end

def rb_aliased_method_to_proc_call_define_method_redefined(value, other)
  value.nil?
end
