String.class_eval do
  def start_with?(prefix)
    false
  end
end

def ruby_class_eval_patch_prefix
  "prelude".start_with?("pre")
end
