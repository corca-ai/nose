iseq = RubyVM::InstructionSequence.compile("define_method(:nil?) { true }")
iseq.eval

def rb_stored_instruction_sequence_eval_string_define_method_redefined(value, other)
  value.nil?
end
