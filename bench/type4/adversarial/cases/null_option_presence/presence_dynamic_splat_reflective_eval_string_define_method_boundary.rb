args = [:eval, "define_method(:nil?) { true }"]
send(*args)

def rb_dynamic_splat_reflective_eval_string_define_method_redefined(value, other)
  value.nil?
end
