[RubyVM::InstructionSequence.compile("define_method(:nil?) { true }")].each { |iseq| iseq.eval }

def rb_instruction_sequence_array_each_block_eval_string_define_method_redefined(value, other)
  value.nil?
end
