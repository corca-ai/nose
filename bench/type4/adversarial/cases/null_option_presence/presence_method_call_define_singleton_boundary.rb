def rb_method_call_define_singleton_redefined(value, other)
  value.method(:define_singleton_method).call(:nil?) { true }
  value.nil?
end
