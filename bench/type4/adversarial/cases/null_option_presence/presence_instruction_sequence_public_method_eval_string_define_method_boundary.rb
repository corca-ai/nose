RubyVM::InstructionSequence.compile("define_method(:nil?) { true }").public_method(:eval).call

def rb_instruction_sequence_public_method_eval_string_define_method_redefined(value, other)
  value.nil?
end
