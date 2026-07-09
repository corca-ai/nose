p = method(:define_method).to_proc.curry

p.call(:nil?) do
  true
end

def rb_to_proc_curry_define_method_redefined(value, other)
  value.nil?
end
