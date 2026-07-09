m = Object.method(:module_eval)
m.call("define_method(:nil?) { true }")

def rb_method_module_eval_string_define_method_redefined(value, other)
  value.nil?
end
