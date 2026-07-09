bin = RubyVM::InstructionSequence.compile("define_method(:nil?) { true }").to_binary
RubyVM::InstructionSequence.load_from_binary(bin).eval

def rb_instruction_sequence_load_from_binary_eval_string_define_method_redefined(value, other)
  value.nil?
end
