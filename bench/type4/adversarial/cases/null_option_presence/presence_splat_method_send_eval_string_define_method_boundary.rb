m = method(:eval)
m.send(*[:call, "define_method(:nil?) { true }"])

def rb_splat_method_send_eval_string_define_method_redefined(value, other)
  value.nil?
end
