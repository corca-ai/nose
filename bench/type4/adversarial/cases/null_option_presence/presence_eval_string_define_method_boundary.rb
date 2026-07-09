eval("define_method(:nil?) { true }")

def rb_eval_string_define_method_redefined(value, other)
  value.nil?
end
