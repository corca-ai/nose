Object.send(:module_eval, "define_method(:nil?) { true }")

def rb_reflective_module_eval_string_define_method_redefined(value, other)
  value.nil?
end
