Object.class_eval("alias_method :nil?, :object_id")

def rb_class_eval_string_alias_method_redefined(value, other)
  value.nil?
end
