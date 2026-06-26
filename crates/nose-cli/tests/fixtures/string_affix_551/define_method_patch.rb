class String
  define_method(:start_with?) do |prefix|
    false
  end
end

def ruby_define_method_patch_prefix
  "prelude".start_with?("pre")
end
