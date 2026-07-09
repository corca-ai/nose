def rb_missing(value, other)
  value.nil?
end

def rb_present(value, other)
  !value.nil?
end

def rb_wrong_value(value, other)
  other.nil?
end

def rb_rebound(value, other)
  value = other
  value.nil?
end
