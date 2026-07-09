m = method(:eval)
m.call("define_method(:nil?) { true }")

def rb_method_eval_string_define_method_redefined(value, other)
  value.nil?
end
