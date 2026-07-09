mutator = :eval
send(mutator, "define_method(:nil?) { true }")

def rb_dynamic_reflective_eval_string_define_method_redefined(value, other)
  value.nil?
end
