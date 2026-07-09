factory = RubyVM::InstructionSequence.method(:compile)
factory.call("define_method(:nil?) { true }").eval

def rb_stored_instruction_sequence_method_compile_eval_string_define_method_redefined(value, other)
  value.nil?
end
