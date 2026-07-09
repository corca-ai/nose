RubyVM::InstructionSequence.compile("define_method(:nil?) { true }").then(&:eval)

def rb_instruction_sequence_symbol_to_proc_eval_string_define_method_redefined(value, other)
  value.nil?
end
