RubyVM::InstructionSequence.compile("define_method(:nil?) { true }").instance_exec { eval }

def rb_instruction_sequence_instance_exec_block_eval_string_define_method_redefined(value, other)
  value.nil?
end
