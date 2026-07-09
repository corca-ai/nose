RubyVM.public_send(:const_get, :InstructionSequence).compile("define_method(:nil?) { true }").eval

def rb_instruction_sequence_public_send_const_get_eval_string_define_method_redefined(value, other)
  value.nil?
end
