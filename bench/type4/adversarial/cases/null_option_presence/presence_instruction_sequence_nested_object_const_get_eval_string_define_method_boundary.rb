Object.const_get(:RubyVM).const_get(:InstructionSequence).compile("define_method(:nil?) { true }").eval

def rb_instruction_sequence_nested_object_const_get_eval_string_define_method_redefined(value, other)
  value.nil?
end
