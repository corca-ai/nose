def rb_reflective_define_singleton_redefined(value, other)
  value.public_send(:define_singleton_method, :nil?) { true }
  value.nil?
end
