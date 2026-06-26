class String
  def start_with?(prefix)
    false
  end
end

def ruby_monkey_patch_prefix
  "prelude".start_with?("pre")
end
