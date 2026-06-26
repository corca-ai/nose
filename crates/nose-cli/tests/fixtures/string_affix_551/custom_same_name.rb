class Token
  def start_with?(prefix)
    prefix == "pre"
  end
end

def ruby_custom_prefix(token)
  token.start_with?("pre")
end
