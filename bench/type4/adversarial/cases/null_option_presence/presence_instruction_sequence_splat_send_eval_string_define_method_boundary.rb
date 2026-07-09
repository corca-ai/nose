RubyVM::InstructionSequence.compile("define_method(:nil?) { true }").send(*[:eval])

def rb_instruction_sequence_splat_send_eval_string_define_method_redefined(value, other)
  value.nil?
end
