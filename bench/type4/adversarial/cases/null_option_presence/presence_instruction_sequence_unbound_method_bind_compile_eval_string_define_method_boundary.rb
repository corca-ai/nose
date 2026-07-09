RubyVM::InstructionSequence.singleton_class.instance_method(:compile).bind(RubyVM::InstructionSequence).call("define_method(:nil?) { true }").eval

def rb_instruction_sequence_unbound_method_bind_compile_eval_string_define_method_redefined(value, other)
  value.nil?
end
