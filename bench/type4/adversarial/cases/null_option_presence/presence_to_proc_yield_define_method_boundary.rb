p = method(:define_method).to_proc

p.yield(:nil?) do
  true
end

def rb_to_proc_yield_define_method_redefined(value, other)
  value.nil?
end
