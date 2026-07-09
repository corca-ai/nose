method(:define_method).then do |m|
  m.call(:nil?) { true }
end

def rb_then_block_method_define_method_redefined(value, other)
  value.nil?
end
