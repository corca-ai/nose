RubyVM::InstructionSequence.compile("define_method(:nil?) { true }").then { |iseq| iseq }.eval

def rb_instruction_sequence_then_eval_string_define_method_redefined(value, other)
  value.nil?
end
