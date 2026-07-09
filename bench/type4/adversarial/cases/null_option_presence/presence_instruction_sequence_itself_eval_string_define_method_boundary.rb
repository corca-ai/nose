RubyVM::InstructionSequence.compile("define_method(:nil?) { true }").itself.eval

def rb_instruction_sequence_itself_eval_string_define_method_redefined(value, other)
  value.nil?
end
