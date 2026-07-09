method(:define_method).tap do |m|
  m.call(:nil?) { true }
end

def rb_tap_block_method_define_method_redefined(value, other)
  value.nil?
end
