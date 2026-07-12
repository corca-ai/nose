def rubyPureCallbackMap(xs)
  xs.map { |value| value }
end

def rubySplatParameterCallbackMap(xs)
  xs.map { |*values| values }
end

def rubyDestructuredParameterCallbackMap(xs)
  xs.map { |(value, _other)| value }
end

def rubyOptionalParameterCallbackMap(xs)
  xs.map { |value = observe| value }
end

def rubyTrailingCommaParameterCallbackMap(xs)
  xs.map { |value,| value }
end

def rubyBlockLocalParameterCallbackMap(xs)
  xs.map { |; value| value }
end

def rubyExplicitReturnCallbackMap(xs)
  xs.map { |value| return value }
end

def rubyMethodDefinitionCallbackMap(xs)
  xs.map do |value|
    def callback_probe
      1
    end
    value
  end
end

def rubyClassDefinitionCallbackMap(xs)
  xs.map do |value|
    class CallbackProbe
    end
    value
  end
end

def rubyInterpolatedRegexCallbackMap(xs)
  xs.map { |value| /#{observe(value)}/ }
end

def rubyWrappedSourceCallbackMap(xs)
  xs.map { |_value| [xs] }
end

def rubyArraySplatCallbackMap(xs)
  xs.map { |_value| [*xs] }
end

def rubyHashCallbackMap(xs)
  xs.map { |value| { "value" => value } }
end
