def rb_stored_method_call_define_singleton_redefined(value, other)
  m = value.method(:define_singleton_method)
  m.call(:nil?) { true }
  value.nil?
end
