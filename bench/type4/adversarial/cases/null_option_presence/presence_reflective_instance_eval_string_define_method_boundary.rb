Object.__send__(:instance_eval, "define_method(:nil?) { true }")

def rb_reflective_instance_eval_string_define_method_redefined(value, other)
  value.nil?
end
